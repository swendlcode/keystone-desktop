//! The decode benchmark. **The first task of the build.**
//!
//! The 0.43 s figure in the engine plan is ffmpeg's hand-optimised MP3 path, and
//! symphonia's 2-4x multiplier over it is an assumption, not a measurement.
//! Decode is roughly 90% of the per-track cost, so at worse than 4x a
//! 20,000-track library crosses two hours and the progress copy, the queue
//! design and the schedule all change.
//!
//! Point `KEYSTONE_BENCH_CORPUS` at a directory of twenty real tracks spanning
//! MP3, AIFF, FLAC and M4A. Without it the benchmark reports that it was skipped
//! rather than inventing a number from synthetic audio, which would measure the
//! wrong thing.
//!
//! Re-measure on a mid-range Windows laptop before any time-remaining estimate
//! is shown to a user: every published timing here is from an M2 Max with eight
//! performance cores, and Windows is roughly two thirds of installs.

use std::path::PathBuf;

use criterion::{criterion_group, criterion_main, Criterion};
use keystone_lib::analysis::decode::{self, DecodeOptions};

fn corpus() -> Vec<PathBuf> {
    let Ok(dir) = std::env::var("KEYSTONE_BENCH_CORPUS") else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .map(|e| decode::is_supported_extension(&e.to_lowercase()))
                .unwrap_or(false)
        })
        .collect()
}

fn decode_corpus(c: &mut Criterion) {
    let files = corpus();
    if files.is_empty() {
        eprintln!("skipped: set KEYSTONE_BENCH_CORPUS to a directory of real tracks");
        return;
    }

    let mut group = c.benchmark_group("decode");
    // Decoding a full track is seconds, not microseconds, so the default
    // sample count would run for an hour.
    group.sample_size(10);
    for path in &files {
        let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        group.bench_function(name, |b| {
            b.iter(|| {
                let opts = DecodeOptions::default();
                decode::decode(path, &opts).map(|d| d.mono_44100.len())
            })
        });
    }
    group.finish();
}

criterion_group!(benches, decode_corpus);
criterion_main!(benches);
