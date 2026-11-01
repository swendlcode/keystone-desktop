//! Rate conversion, streaming and one-shot.
//!
//! The streaming form is what keeps S1 a single pass: the decode loop hands it
//! source-rate mono as the packets arrive and gets analysis-rate mono back in
//! chunks, so the waveform peaks can be accumulated inside the loop and the
//! source-rate signal is never retained.
//!
//! Two properties are worth the extra code over a one-shot call per track.
//! **The start-up delay is removed**, because a resampler emits roughly half its
//! block as leading silence and anything that keeps it hands the beat grid a
//! constant offset against the file. **The output length is exact**: the
//! last chunk has to be flushed with silence, and the padding that flush
//! produces is trimmed before it reaches the caller — otherwise the cached
//! waveform runs past the end of the audio and the player's clock and the
//! drawn waveform disagree visibly.

use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{
    Async, Fft, FixedAsync, FixedSync, Indexing, Resampler, SincInterpolationParameters,
    SincInterpolationType, WindowFunction,
};

use crate::error::{KeystoneError, Result};

/// Input frames per call. Small enough that the converter adds no meaningful
/// latency to the decode loop, large enough that the per-call overhead is
/// nothing next to the sinc itself.
const CHUNK: usize = 1024;

/// Only the fallback path uses these. A multiple of 8 is a hard requirement of
/// rubato's interpolator; the cutoff is left to rubato, which derives a safe one
/// from the ratio, because hard-coding one aliases on every downward conversion
/// and 48 kHz to 44.1 kHz is the common case.
fn params() -> SincInterpolationParameters {
    SincInterpolationParameters {
        sinc_len: 128,
        f_cutoff: None,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 256,
        window: WindowFunction::BlackmanHarris2,
    }
}

/// Both rates are always integers here, so every conversion the engine asks
/// for is a rational one and belongs on rubato's synchronous FFT resampler
/// rather than its asynchronous sinc one. Measured on the 44.1 kHz to 22.05 kHz
/// pass of a 5-minute track: **0.055 s** through the FFT resampler against
/// **0.18 s** through the sinc at 128 taps, on a decode that is itself 0.6-1.4 s.
///
/// The sinc path stays as the fallback for a rate pair the FFT resampler
/// refuses to build — it accepts any ratio, including an irrational one, which
/// is what a tuning correction would want if it ever asked for one directly.
fn build(from: u32, to: u32) -> Result<Box<dyn Resampler<f32>>> {
    if let Ok(fft) = Fft::<f32>::new(from as usize, to as usize, CHUNK, 1, FixedSync::Input) {
        return Ok(Box::new(fft));
    }
    let ratio = f64::from(to) / f64::from(from);
    let sinc = Async::<f32>::new_sinc(ratio, 1.1, &params(), CHUNK, 1, FixedAsync::Input)
        .map_err(|e| KeystoneError::Decode(format!("resampler {from} -> {to} Hz: {e}")))?;
    Ok(Box::new(sinc))
}

/// Mono rate conversion. A pass-through when the rates already match, which is
/// the common case in a DJ library and must cost nothing.
pub struct Converter {
    resampler: Option<Box<dyn Resampler<f32>>>,
    ratio: f64,
    pending: Vec<f32>,
    chunk: Vec<f32>,
    scratch: Vec<f32>,
    /// Output frames of start-up silence still to be dropped.
    skip: usize,
    in_total: u64,
    out_total: u64,
}

impl Converter {
    pub fn new(from: u32, to: u32) -> Result<Self> {
        if from == 0 || to == 0 {
            return Err(KeystoneError::Decode(format!(
                "cannot resample {from} Hz to {to} Hz"
            )));
        }
        if from == to {
            return Ok(Self::passthrough());
        }

        let ratio = f64::from(to) / f64::from(from);
        let resampler = build(from, to)?;
        let skip = resampler.output_delay();

        Ok(Self {
            resampler: Some(resampler),
            ratio,
            pending: Vec::with_capacity(CHUNK * 2),
            chunk: vec![0.0; CHUNK],
            scratch: Vec::new(),
            skip,
            in_total: 0,
            out_total: 0,
        })
    }

    fn passthrough() -> Self {
        Self {
            resampler: None,
            ratio: 1.0,
            pending: Vec::new(),
            chunk: Vec::new(),
            scratch: Vec::new(),
            skip: 0,
            in_total: 0,
            out_total: 0,
        }
    }

    /// Converts what it can and appends the result to `out`. The caller reads
    /// the newly appended tail to feed the peak accumulator, which is what
    /// makes the peaks streaming rather than a second pass.
    pub fn push(&mut self, input: &[f32], out: &mut Vec<f32>) -> Result<()> {
        if self.resampler.is_none() {
            self.in_total += input.len() as u64;
            self.out_total += input.len() as u64;
            out.extend_from_slice(input);
            return Ok(());
        }

        self.in_total += input.len() as u64;
        let mut rest = input;
        loop {
            let needed = self.needed();
            // Whole chunks are taken straight off the caller's slice. Queueing
            // the whole packet and draining the front of the queue instead is
            // O(n^2) over a track — measured at 7.5 s for the 22.05 kHz pass of
            // a 5-minute file, which looked exactly like a slow resampler.
            if self.pending.is_empty() && rest.len() >= needed {
                self.chunk.clear();
                self.chunk.extend_from_slice(&rest[..needed]);
                rest = &rest[needed..];
                self.run(None, out)?;
                continue;
            }
            let take = (needed - self.pending.len()).min(rest.len());
            self.pending.extend_from_slice(&rest[..take]);
            rest = &rest[take..];
            if self.pending.len() < needed {
                // Under a chunk: it waits for the next packet.
                return Ok(());
            }
            self.chunk.clear();
            self.chunk.append(&mut self.pending);
            self.run(None, out)?;
        }
    }

    /// Flushes the tail and trims the padding the flush produced, so the output
    /// is exactly the length the input rate implies.
    pub fn finish(&mut self, out: &mut Vec<f32>) -> Result<()> {
        if self.resampler.is_none() {
            return Ok(());
        }
        let expected = (self.in_total as f64 * self.ratio).round() as u64;

        // Bounded because each pass consumes a whole chunk of input time: with
        // the pending remainder under one chunk and a delay under one kernel,
        // the flush cannot legitimately need many passes.
        for _ in 0..8 {
            if self.out_total >= expected {
                break;
            }
            let needed = self.needed();
            let valid = self.pending.len().min(needed);
            self.chunk.clear();
            self.chunk.append(&mut self.pending);
            self.chunk.truncate(valid);
            self.chunk.resize(needed, 0.0);
            self.run(Some(valid), out)?;
        }

        if self.out_total > expected {
            let excess = (self.out_total - expected) as usize;
            out.truncate(out.len().saturating_sub(excess));
            self.out_total = expected;
        }
        Ok(())
    }

    fn needed(&self) -> usize {
        self.resampler
            .as_ref()
            .map(|r| r.input_frames_next())
            .unwrap_or(CHUNK)
    }

    /// One resampler call over `self.chunk`, with `valid` set when the chunk is
    /// a partial final one and the rest is silence.
    fn run(&mut self, valid: Option<usize>, out: &mut Vec<f32>) -> Result<()> {
        let Some(resampler) = self.resampler.as_mut() else {
            return Ok(());
        };
        let in_frames = resampler.input_frames_next();
        let out_frames = resampler.output_frames_next();
        self.scratch.resize(out_frames, 0.0);

        let input = InterleavedSlice::new(&self.chunk[..], 1, in_frames)
            .map_err(|e| KeystoneError::Decode(format!("resampler input: {e}")))?;
        let mut sink = InterleavedSlice::new_mut(&mut self.scratch[..], 1, out_frames)
            .map_err(|e| KeystoneError::Decode(format!("resampler output: {e}")))?;
        let indexing = Indexing {
            partial_len: valid,
            ..Default::default()
        };
        let (_, produced) = resampler
            .process_into_buffer(&input, &mut sink, Some(&indexing))
            .map_err(|e| KeystoneError::Decode(format!("resampling failed: {e}")))?;

        let skipped = self.skip.min(produced);
        self.skip -= skipped;
        let kept = &self.scratch[skipped..produced];
        out.extend_from_slice(kept);
        self.out_total += kept.len() as u64;
        Ok(())
    }
}

/// One-shot conversion, over the same streaming path so the delay trimming and
/// the length arithmetic have one implementation rather than two.
pub fn resample(samples: &[f32], from: u32, to: u32) -> Result<Vec<f32>> {
    if from == to {
        return Ok(samples.to_vec());
    }
    let mut converter = Converter::new(from, to)?;
    if samples.is_empty() {
        return Ok(Vec::new());
    }

    let expected = (samples.len() as f64 * f64::from(to) / f64::from(from)).round() as usize;
    let mut out = Vec::with_capacity(expected + CHUNK);
    converter.push(samples, &mut out)?;
    converter.finish(&mut out)?;
    Ok(out)
}
