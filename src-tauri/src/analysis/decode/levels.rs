//! Peak, true peak, integrated loudness and clipped samples, measured in the
//! decode loop.
//!
//! These are measured on the **source** channels at the source rate, not on the
//! analysis-rate mono sum: a downmix hides inter-channel peaks, and a resample
//! creates its own. What is reported has to be a fact about the user's file,
//! because it is the number they will compare against their mastering engineer.

use ebur128::{EbuR128, Mode};

use crate::models::LevelsResult;

/// 16-bit full scale is 32767/32768, so a decoded sample never reaches 1.0 on
/// the format where clipping is most common. An exact test for 1.0 would report
/// zero clipped samples on a brickwalled CD rip.
const CLIP_FLOOR: f32 = 0.999_9;

/// Digital silence is -inf dB, and the analysis cache is JSON, which has no
/// infinity: serializing one would write `null` into the payload and fail the
/// next read. A floor well below anything audible keeps the row readable.
const SILENCE_DB: f64 = -120.0;

pub struct LevelMeter {
    /// `None` when the meter refused the channel count or rate. Levels are a
    /// detail of the result, so losing them must not fail the analysis.
    meter: Option<EbuR128>,
    channels: u32,
    peak: f32,
    clipped: i64,
}

impl LevelMeter {
    pub fn new(channels: u16, sample_rate: u32) -> Self {
        let channels = u32::from(channels).max(1);
        let meter = match EbuR128::new(channels, sample_rate, Mode::I | Mode::TRUE_PEAK) {
            Ok(m) => Some(m),
            Err(e) => {
                tracing::debug!("loudness meter unavailable ({channels} ch, {sample_rate} Hz): {e}");
                None
            }
        };
        Self {
            meter,
            channels,
            peak: 0.0,
            clipped: 0,
        }
    }

    /// Interleaved source-rate frames, exactly as decoded.
    pub fn push(&mut self, interleaved: &[f32]) {
        for &s in interleaved {
            let a = s.abs();
            if a > self.peak {
                self.peak = a;
            }
            if a >= CLIP_FLOOR {
                self.clipped += 1;
            }
        }
        if let Some(meter) = self.meter.as_mut() {
            // A short frame at the end of a stream is the one thing the meter
            // rejects outright; dropping the measurement is better than
            // dropping the track.
            if let Err(e) = meter.add_frames_f32(interleaved) {
                tracing::debug!("loudness frames rejected: {e}");
                self.meter = None;
            }
        }
    }

    pub fn finish(self) -> LevelsResult {
        let peak_db = to_db(f64::from(self.peak));
        let (true_peak_db, lufs_integrated) = match self.meter.as_ref() {
            Some(meter) => {
                let tp = (0..self.channels)
                    .filter_map(|c| meter.true_peak(c).ok())
                    .fold(0.0f64, f64::max);
                // The oversampled peak can only exceed the sample peak; if the
                // meter reported nothing useful, the sample peak is the honest
                // floor for it rather than zero.
                let tp_db = to_db(tp.max(f64::from(self.peak)));
                let lufs = meter
                    .loudness_global()
                    .ok()
                    .filter(|v| v.is_finite())
                    .unwrap_or(SILENCE_DB);
                (tp_db, lufs.max(SILENCE_DB))
            }
            None => (peak_db, SILENCE_DB),
        };

        LevelsResult {
            peak_db,
            true_peak_db,
            lufs_integrated,
            clipped_peaks: self.clipped,
        }
    }
}

fn to_db(amplitude: f64) -> f64 {
    if amplitude <= 0.0 {
        return SILENCE_DB;
    }
    (20.0 * amplitude.log10()).max(SILENCE_DB)
}
