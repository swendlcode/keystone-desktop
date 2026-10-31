//! The decode benchmark over generated fixtures.
//!
//! Companion to `decode.rs`, which measures a real corpus and skips itself when
//! there is not one. This one always runs, because the number it produces is the
//! one the engine plan says to settle first: the 0.43 s figure in §2 is ffmpeg's
//! hand-optimised MP3 path and symphonia's 2-4x multiplier over it is an
//! assumption. Decode is roughly 90% of the per-track cost, so at worse than 4x
//! a 20,000-track library crosses two hours and the progress copy, the queue
//! design and the schedule all change.
//!
//! **Throughput is reported in audio-seconds per wall-second, which is the
//! realtime multiplier directly.** Divide the plan's per-track budget by it.
//!
//! Only WAV and AIFF are generated. MP3, FLAC, M4A and ALAC need encoders, and
//! Keystone depends on none — symphonia decodes only. Rather than guess at
//! those formats, the honest split is: this benchmark bounds the PCM container
//! path and the shared downmix, resample, peak and level work, and `decode.rs`
//! measures the compressed codecs on real files. The MP3 multiplier in
//! particular stays unmeasured until somebody points `KEYSTONE_BENCH_CORPUS` at
//! a folder of tracks.
//!
//! Re-measure on a mid-range Windows laptop before any time-remaining estimate
//! is shown to a user: every published timing is from an M2 Max with eight
//! performance cores, and Windows is roughly two thirds of installs.

use std::f32::consts::TAU;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use keystone_lib::analysis::decode::{self, DecodeOptions};

const SAMPLE_RATE: u32 = 44_100;
const CHANNELS: u16 = 2;

/// Long enough that per-call overhead does not dominate, short enough that four
/// fixtures fit in a temp directory without thought. A decode is linear in
/// length, so the multiplier does not depend on this.
const DEFAULT_SECONDS: u32 = 60;

fn seconds() -> u32 {
    std::env::var("KEYSTONE_BENCH_SECONDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|s| *s > 0)
        .unwrap_or(DEFAULT_SECONDS)
}

/// Deterministic, and deliberately not a single sine: three partials plus a
/// little shaped noise, so the sample values span the range and a decoder
/// cannot take a trivial path through them.
fn signal(frames: usize) -> Vec<f32> {
    let mut out = Vec::with_capacity(frames * CHANNELS as usize);
    let mut lcg: u32 = 0x2545_F491;
    for i in 0..frames {
        let t = i as f32 / SAMPLE_RATE as f32;
        lcg = lcg.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let noise = (lcg >> 8) as f32 / 8_388_608.0 - 1.0;
        let tone = 0.45 * (TAU * 110.0 * t).sin()
            + 0.25 * (TAU * 440.0 * t).sin()
            + 0.12 * (TAU * 3_300.0 * t).sin();
        let left = (tone + 0.05 * noise).clamp(-1.0, 1.0);
        // A little channel difference, so the downmix is not a no-op.
        let right = (tone * 0.85 - 0.05 * noise).clamp(-1.0, 1.0);
        out.push(left);
        out.push(right);
    }
    out
}

fn wav(dir: &Path, name: &str, samples: &[f32], bits: u16, float: bool) -> PathBuf {
    let bytes_per_sample = (bits / 8) as u32;
    let data: Vec<u8> = samples
        .iter()
        .flat_map(|s| encode_sample(*s, bits, float))
        .collect();
    let block_align = bytes_per_sample as u16 * CHANNELS;
    let mut out = Vec::with_capacity(data.len() + 44);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&if float { 3u16 } else { 1u16 }.to_le_bytes());
    out.extend_from_slice(&CHANNELS.to_le_bytes());
    out.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    out.extend_from_slice(&(SAMPLE_RATE * block_align as u32).to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&bits.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(&data);
    write(dir, name, &out)
}

fn encode_sample(s: f32, bits: u16, float: bool) -> Vec<u8> {
    if float {
        return s.to_le_bytes().to_vec();
    }
    match bits {
        24 => {
            let v = (s * 8_388_607.0) as i32;
            v.to_le_bytes()[..3].to_vec()
        }
        _ => ((s * 32_767.0) as i16).to_le_bytes().to_vec(),
    }
}

/// AIFF is big-endian throughout, and its sample rate is an 80-bit IEEE 754
/// extended float — the one field that cannot be written by casting.
fn aiff(dir: &Path, name: &str, samples: &[f32]) -> PathBuf {
    let data: Vec<u8> = samples
        .iter()
        .flat_map(|s| ((s * 32_767.0) as i16).to_be_bytes())
        .collect();
    let frames = (samples.len() / CHANNELS as usize) as u32;
    let ssnd_len = 8 + data.len() as u32;
    let mut out = Vec::with_capacity(data.len() + 54);
    out.extend_from_slice(b"FORM");
    out.extend_from_slice(&(4 + 8 + 18 + 8 + ssnd_len).to_be_bytes());
    out.extend_from_slice(b"AIFFCOMM");
    out.extend_from_slice(&18u32.to_be_bytes());
    out.extend_from_slice(&CHANNELS.to_be_bytes());
    out.extend_from_slice(&frames.to_be_bytes());
    out.extend_from_slice(&16u16.to_be_bytes());
    // 44,100 Hz as an 80-bit extended float: exponent 0x400E, mantissa
    // 0xAC44000000000000.
    out.extend_from_slice(&[0x40, 0x0E, 0xAC, 0x44, 0, 0, 0, 0, 0, 0]);
    out.extend_from_slice(b"SSND");
    out.extend_from_slice(&ssnd_len.to_be_bytes());
    out.extend_from_slice(&0u32.to_be_bytes()); // offset
    out.extend_from_slice(&0u32.to_be_bytes()); // block size
    out.extend_from_slice(&data);
    write(dir, name, &out)
}

fn write(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let path = dir.join(name);
    let mut file = fs::File::create(&path).expect("the fixture could not be written");
    file.write_all(bytes).expect("the fixture is incomplete");
    path
}

fn decode_generated(c: &mut Criterion) {
    let secs = seconds();
    let dir = tempfile::tempdir().expect("a temp directory for the fixtures");
    let samples = signal(secs as usize * SAMPLE_RATE as usize);
    let fixtures = [
        wav(dir.path(), "pcm16.wav", &samples, 16, false),
        wav(dir.path(), "pcm24.wav", &samples, 24, false),
        wav(dir.path(), "f32.wav", &samples, 32, true),
        aiff(dir.path(), "pcm16.aiff", &samples),
    ];

    let mut group = c.benchmark_group("decode_generated");
    // A full decode is seconds, not microseconds, so the default sample count
    // would run for the better part of an hour.
    group.sample_size(10);
    // Audio-seconds per wall-second: the realtime multiplier, read straight off
    // the report rather than divided out by hand afterwards.
    group.throughput(Throughput::Elements(secs as u64));
    for path in &fixtures {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        assert!(
            decode::is_supported_extension(
                &path
                    .extension()
                    .map(|e| e.to_string_lossy().to_lowercase())
                    .unwrap_or_default()
            ),
            "{name} is not an extension the scanner would pick up"
        );
        group.bench_function(format!("{name}-{secs}s"), |b| {
            b.iter(|| {
                let opts = DecodeOptions::default();
                decode::decode(path, &opts).map(|d| d.mono_44100.len())
            })
        });
    }
    group.finish();
}

criterion_group!(benches, decode_generated);
criterion_main!(benches);
