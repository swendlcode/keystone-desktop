//! The one streaming decode pass.
//!
//! Everything the pipeline needs off the audio comes out of this single pass:
//! the analysis-rate mono signal, the key model's 22.05 kHz signal, the levels
//! and the waveform peaks. Decode is roughly 90% of the per-track cost, so a
//! second pass over the same file would be the most expensive mistake available
//! at this layer — nothing here may be computed by decoding twice.
//!
//! Peaks are accumulated from the converted chunks as they are produced, so the
//! accumulator stays O(target buckets) and the source-rate signal is never
//! retained.

use std::path::Path;

use symphonia::core::audio::{AudioBufferRef, SampleBuffer, SignalSpec};
use symphonia::core::codecs::{Decoder, DecoderOptions};
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::FormatReader;

use super::levels::LevelMeter;
use super::rate::{self, Converter};
use super::{open, AudioProbe, DecodeOptions, DecodedAudio};
use super::{ANALYSIS_SAMPLE_RATE, KEY_SAMPLE_RATE};
use crate::core::peaks::PeakAccumulator;
use crate::error::Result;

/// A damaged frame is normal in a DJ library — a bad rip, a part-downloaded
/// promo — and dropping one is better than refusing the track. A run this long
/// is a damaged *file*, and continuing would be a scan that never ends.
const MAX_CONSECUTIVE_ERRORS: u32 = 64;

pub fn decode(path: &Path, opts: &DecodeOptions) -> Result<DecodedAudio> {
    let opened = open::open(path)?;
    let mut probe = open::describe(path, &opened)?;
    let open::Opened {
        reader,
        track_id,
        params,
        ..
    } = opened;

    let decoder = symphonia::default::get_codecs()
        .make(&params, &DecoderOptions { verify: false })
        .map_err(|e| open::fail(path, &format!("no decoder for {} ({e})", probe.codec)))?;

    let active = run(reader, decoder, track_id, opts)?
        .filter(|a| !a.is_empty())
        .ok_or_else(|| open::fail(path, "no audio frames could be decoded"))?;

    // What the pass counted beats what the header claimed: a truncated file
    // declares the length it was meant to have, and every later stage times
    // itself against the samples that actually exist.
    active.finish(&mut probe, opts)
}

/// The packet loop. `None` means the stream yielded no audio at all, which is
/// the caller's error to report with the path in it.
fn run(
    mut reader: Box<dyn FormatReader>,
    mut decoder: Box<dyn Decoder>,
    track_id: u32,
    opts: &DecodeOptions,
) -> Result<Option<Active>> {
    let mut pass: Option<Active> = None;
    let mut sample_buf: Option<SampleBuffer<f32>> = None;
    let mut errors = 0u32;

    loop {
        let packet = match reader.next_packet() {
            Ok(packet) => packet,
            // End of stream, and also where a truncated file lands: the frames
            // that did decode are a usable result, and refusing them would
            // make a half-copied file unanalysable rather than imperfect.
            Err(SymError::IoError(_)) | Err(SymError::ResetRequired) => break,
            Err(SymError::DecodeError(_)) if errors < MAX_CONSECUTIVE_ERRORS => {
                errors += 1;
                continue;
            }
            Err(_) => break,
        };
        if packet.track_id() != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(audio) => {
                errors = 0;
                accept(&mut pass, &mut sample_buf, audio, opts)?;
            }
            Err(SymError::ResetRequired) => decoder.reset(),
            Err(SymError::DecodeError(_)) | Err(SymError::IoError(_))
                if errors < MAX_CONSECUTIVE_ERRORS =>
            {
                errors += 1;
            }
            Err(_) => break,
        }
    }
    Ok(pass)
}

fn accept(
    pass: &mut Option<Active>,
    sample_buf: &mut Option<SampleBuffer<f32>>,
    audio: AudioBufferRef,
    opts: &DecodeOptions,
) -> Result<()> {
    let spec = *audio.spec();
    if audio.frames() == 0 {
        return Ok(());
    }
    let interleaved = interleave(sample_buf, audio);
    if pass.is_none() {
        // Built from the decoded signal's own spec rather than the header's: a
        // container that disagrees with its stream is a real file, and the
        // samples in hand are the ones that have to be converted correctly.
        *pass = Some(Active::new(spec, opts)?);
    }
    match pass.as_mut() {
        Some(active) => active.consume(interleaved),
        None => Ok(()),
    }
}

/// Converts one decoded buffer to interleaved f32, reusing the allocation
/// across packets. Sample format conversion lives in symphonia; this is only
/// the buffer management around it.
fn interleave<'a>(
    slot: &'a mut Option<SampleBuffer<f32>>,
    audio: AudioBufferRef,
) -> &'a [f32] {
    let spec = *audio.spec();
    let frames = audio.frames();
    let needed = frames * spec.channels.count().max(1);
    let fits = slot.as_ref().map(|b| b.capacity() >= needed).unwrap_or(false);
    if !fits {
        let duration = audio.capacity().max(frames) as u64;
        *slot = Some(SampleBuffer::new(duration, spec));
    }
    match slot.as_mut() {
        Some(buf) => {
            buf.copy_interleaved_ref(audio);
            buf.samples()
        }
        None => &[],
    }
}

/// The state of a pass in flight, created once the first buffer has told us
/// what the audio really is.
struct Active {
    channels: usize,
    source_rate: u32,
    converter: Converter,
    meter: LevelMeter,
    peaks: Option<PeakAccumulator>,
    mono: Vec<f32>,
    scratch: Vec<f32>,
    source_frames: u64,
}

impl Active {
    fn new(spec: SignalSpec, opts: &DecodeOptions) -> Result<Self> {
        let channels = spec.channels.count().max(1);
        let peaks = opts.want_peaks.then(|| {
            // At the analysis rate, because that is what the zoom levels are
            // defined in and what the player maps time with.
            PeakAccumulator::new(&opts.peak_zoom_levels, ANALYSIS_SAMPLE_RATE)
        });
        Ok(Self {
            channels,
            source_rate: spec.rate,
            converter: Converter::new(spec.rate, ANALYSIS_SAMPLE_RATE)?,
            meter: LevelMeter::new(channels as u16, spec.rate),
            peaks,
            // A minute up front: the common case allocates a handful of times
            // instead of growing through a 5-minute track.
            mono: Vec::with_capacity(ANALYSIS_SAMPLE_RATE as usize * 60),
            scratch: Vec::new(),
            source_frames: 0,
        })
    }

    fn consume(&mut self, interleaved: &[f32]) -> Result<()> {
        self.meter.push(interleaved);
        self.source_frames += (interleaved.len() / self.channels) as u64;

        self.scratch.clear();
        downmix(interleaved, self.channels, &mut self.scratch);

        let before = self.mono.len();
        self.converter.push(&self.scratch, &mut self.mono)?;
        if let Some(peaks) = self.peaks.as_mut() {
            peaks.push(&self.mono[before..]);
        }
        Ok(())
    }

    fn is_empty(&self) -> bool {
        self.source_frames == 0
    }

    fn finish(mut self, probe: &mut AudioProbe, opts: &DecodeOptions) -> Result<DecodedAudio> {
        let before = self.mono.len();
        self.converter.finish(&mut self.mono)?;
        if let Some(peaks) = self.peaks.as_mut() {
            if self.mono.len() > before {
                peaks.push(&self.mono[before..]);
            }
        }

        let duration_ms = (self.source_frames as i64 * 1000) / i64::from(self.source_rate.max(1));
        probe.duration_ms = Some(duration_ms);
        probe.n_frames = Some(self.source_frames);

        // The key rate is an exact 2:1 decimation of the analysis signal, so it
        // costs a filter pass and no second decode.
        let mono_22050 = match opts.want_key_rate {
            true => Some(rate::resample(
                &self.mono,
                ANALYSIS_SAMPLE_RATE,
                KEY_SAMPLE_RATE,
            )?),
            false => None,
        };

        Ok(DecodedAudio {
            mono_44100: self.mono,
            mono_22050,
            probe: probe.clone(),
            duration_ms,
            levels: self.meter.finish(),
            peaks: self.peaks.map(|p| p.finish()),
        })
    }
}

/// Arithmetic mean, as the plan specifies. Not a weighted or power-preserving
/// downmix: the chroma and onset stages are calibrated against this one.
fn downmix(interleaved: &[f32], channels: usize, out: &mut Vec<f32>) {
    if channels <= 1 {
        out.extend_from_slice(interleaved);
        return;
    }
    let inv = 1.0 / channels as f32;
    out.reserve(interleaved.len() / channels);
    for frame in interleaved.chunks_exact(channels) {
        out.push(frame.iter().sum::<f32>() * inv);
    }
}
