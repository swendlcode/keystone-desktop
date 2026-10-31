//! Waveform peaks, accumulated inside the decode loop and cached on disk.
//!
//! Peaks never go into the `tracks` row. Keystone's tracks are full length with
//! several zoom levels, and the virtualized table selects that row on every
//! page — so a peak blob in the row costs a megabyte of SELECT per screen. They
//! live under app data keyed by `content_hash`, which is also exactly what the
//! spec's "Cache audio wave image during analysis" setting means.

mod cursor;

use std::path::{Path, PathBuf};

use cursor::Cursor;

use crate::error::{KeystoneError, Result};
use crate::models::{Waveform, WaveformLevel};

/// Samples per bucket, at 44.1 kHz. The widest level is the whole-track
/// overview the player bar draws; the narrowest supports the zoom `+` button
/// without a re-decode.
pub const ZOOM_LEVELS: [u32; 4] = [8192, 2048, 512, 128];

/// `KSPK` plus a format byte. Bumping the byte makes every cached file a miss
/// rather than a misread, which is the only safe way to change the layout.
const MAGIC: [u8; 4] = *b"KSPK";
const FORMAT_VERSION: u8 = 1;

pub struct PeakLevel {
    pub samples_per_bucket: u32,
    pub min: Vec<f32>,
    pub max: Vec<f32>,
}

pub struct PeakPyramid {
    pub sample_rate: u32,
    /// Counted as the decode loop ran, which is the only honest source for it.
    ///
    /// Deriving it from `buckets x samples_per_bucket` would round up to the
    /// padded final bucket and report a 1.000 s track as 1.010 s, so the cached
    /// waveform and the player's clock would disagree by a visible amount.
    /// [`PeakAccumulator::finish`] is what fills this in.
    pub total_samples: u64,
    pub levels: Vec<PeakLevel>,
}

/// One level's running bucket, kept as a pair rather than a Vec of blocks so
/// memory is O(buckets) and never O(samples).
struct LevelAccumulator {
    samples_per_bucket: u32,
    filled: u32,
    min: f32,
    max: f32,
    mins: Vec<f32>,
    maxes: Vec<f32>,
}

impl LevelAccumulator {
    fn new(samples_per_bucket: u32, sample_rate: u32) -> Self {
        // Enough for five minutes up front: the common case allocates once and
        // a longer track simply grows.
        let expected = (sample_rate as u64 * 300 / samples_per_bucket.max(1) as u64) as usize;
        Self {
            samples_per_bucket: samples_per_bucket.max(1),
            filled: 0,
            min: f32::MAX,
            max: f32::MIN,
            mins: Vec::with_capacity(expected),
            maxes: Vec::with_capacity(expected),
        }
    }

    fn push(&mut self, samples: &[f32]) {
        for &s in samples {
            if s < self.min {
                self.min = s;
            }
            if s > self.max {
                self.max = s;
            }
            self.filled += 1;
            if self.filled >= self.samples_per_bucket {
                self.close();
            }
        }
    }

    fn close(&mut self) {
        self.mins.push(if self.min == f32::MAX { 0.0 } else { self.min });
        self.maxes.push(if self.max == f32::MIN { 0.0 } else { self.max });
        self.filled = 0;
        self.min = f32::MAX;
        self.max = f32::MIN;
    }
}

/// Streaming accumulator: O(target buckets) in memory, never O(samples). Fed
/// from inside the decode loop, so a 10-minute file is never held as a buffer.
pub struct PeakAccumulator {
    sample_rate: u32,
    total_samples: u64,
    levels: Vec<LevelAccumulator>,
}

impl PeakAccumulator {
    pub fn new(levels: &[u32], sample_rate: u32) -> Self {
        Self {
            sample_rate,
            total_samples: 0,
            levels: levels
                .iter()
                .map(|n| LevelAccumulator::new(*n, sample_rate))
                .collect(),
        }
    }

    pub fn push(&mut self, samples: &[f32]) {
        self.total_samples += samples.len() as u64;
        for level in &mut self.levels {
            level.push(samples);
        }
    }

    pub fn finish(mut self) -> PeakPyramid {
        for level in &mut self.levels {
            // The tail of a track is a partial bucket; dropping it would cut up
            // to 8191 samples off the drawn waveform's end.
            if level.filled > 0 {
                level.close();
            }
        }
        PeakPyramid {
            sample_rate: self.sample_rate,
            total_samples: self.total_samples,
            levels: self
                .levels
                .into_iter()
                .map(|l| PeakLevel {
                    samples_per_bucket: l.samples_per_bucket,
                    min: l.mins,
                    max: l.maxes,
                })
                .collect(),
        }
    }
}

pub fn cache_path(dir: &Path, content_hash: &str) -> PathBuf {
    // Two-character shard, so a 20,000-track library does not put 20,000
    // entries in one directory.
    let (shard, rest) = content_hash.split_at(content_hash.len().min(2));
    dir.join(shard).join(format!("{rest}.peaks"))
}

pub fn write(dir: &Path, content_hash: &str, pyramid: &PeakPyramid) -> Result<()> {
    let path = cache_path(dir, content_hash);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut buf = Vec::new();
    buf.extend_from_slice(&MAGIC);
    buf.push(FORMAT_VERSION);
    buf.extend_from_slice(&pyramid.sample_rate.to_le_bytes());
    buf.extend_from_slice(&pyramid.total_samples.to_le_bytes());
    buf.push(pyramid.levels.len() as u8);
    for level in &pyramid.levels {
        buf.extend_from_slice(&level.samples_per_bucket.to_le_bytes());
        buf.extend_from_slice(&(level.min.len() as u32).to_le_bytes());
        for v in level.min.iter().chain(level.max.iter()) {
            buf.extend_from_slice(&v.to_le_bytes());
        }
    }

    // Same directory, then rename: a crash mid-write must leave either the old
    // file or none, never a half-written pyramid that reads as a waveform.
    let tmp = tempfile::NamedTempFile::new_in(path.parent().unwrap_or(dir))?;
    std::fs::write(tmp.path(), &buf)?;
    tmp.persist(&path)
        .map_err(|e| KeystoneError::Io(e.error))?;
    Ok(())
}

pub fn read(dir: &Path, content_hash: &str) -> Result<Option<Waveform>> {
    let path = cache_path(dir, content_hash);
    let Ok(bytes) = std::fs::read(&path) else {
        return Ok(None);
    };
    // A cache file written by another format version, or truncated by a crash
    // on a machine without atomic rename, is a miss. Never an error: the whole
    // point of a cache is that losing it costs one decode.
    Ok(decode_pyramid(&bytes, content_hash))
}

fn decode_pyramid(bytes: &[u8], content_hash: &str) -> Option<Waveform> {
    let mut c = Cursor::new(bytes);
    if c.take(4)? != MAGIC || c.u8()? != FORMAT_VERSION {
        return None;
    }
    let sample_rate = c.u32()?;
    let total_samples = c.u64()?;
    let level_count = c.u8()? as usize;

    let mut levels = Vec::with_capacity(level_count);
    for _ in 0..level_count {
        let samples_per_bucket = c.u32()?;
        let buckets = c.u32()? as usize;
        let min = c.f32s(buckets)?;
        let max = c.f32s(buckets)?;
        levels.push(WaveformLevel {
            samples_per_bucket,
            min,
            max,
        });
    }

    Some(Waveform {
        content_hash: content_hash.to_string(),
        duration_ms: if sample_rate > 0 {
            (total_samples as i64 * 1000) / sample_rate as i64
        } else {
            0
        },
        sample_rate,
        levels,
    })
}

/// Drops cached files nothing references any more. Called by Clean cache.
pub fn prune(dir: &Path, live_hashes: &[String]) -> Result<(usize, u64)> {
    let live: std::collections::HashSet<&str> = live_hashes.iter().map(String::as_str).collect();
    let mut removed = 0usize;
    let mut freed = 0u64;

    let Ok(shards) = std::fs::read_dir(dir) else {
        return Ok((0, 0));
    };
    for shard in shards.flatten() {
        if !shard.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let prefix = shard.file_name().to_string_lossy().into_owned();
        let Ok(entries) = std::fs::read_dir(shard.path()) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Some(rest) = name.strip_suffix(".peaks") else {
                continue;
            };
            if live.contains(format!("{prefix}{rest}").as_str()) {
                continue;
            }
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            if std::fs::remove_file(entry.path()).is_ok() {
                removed += 1;
                freed += size;
            }
        }
    }
    Ok((removed, freed))
}

#[cfg(test)]
#[path = "peaks_tests.rs"]
mod tests;
