use std::fs;

use super::fixtures as fx;
use super::*;

fn dir() -> tempfile::TempDir {
    tempfile::tempdir().expect("a temp directory for the fixtures")
}

fn head(path: &std::path::Path, offset: u64, len: usize) -> Vec<u8> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = fs::File::open(path).expect("open");
    f.seek(SeekFrom::Start(offset)).expect("seek");
    let mut buf = vec![0u8; len];
    f.read_exact(&mut buf).expect("read");
    buf
}

#[test]
fn probe_reports_the_header_facts() {
    let d = dir();
    let pcm = fx::tone(440.0, 0.5, 1.0, 44_100, 2);
    let path = fx::wav(d.path(), "a.wav", &pcm, 16, false);

    let p = probe(&path).expect("probe");
    assert_eq!(p.codec, "pcm_s16le");
    assert_eq!(p.sample_rate, 44_100);
    assert_eq!(p.channels, 2);
    assert_eq!(p.bit_depth, Some(16));
    assert_eq!(p.n_frames, Some(44_100));
    assert_eq!(p.duration_ms, Some(1_000));
    // 44,100 x 2 ch x 16 bit = 1,411 kbps, plus the 44-byte header.
    let kbps = p.bitrate_kbps.expect("a bitrate");
    assert!((1_400..=1_420).contains(&kbps), "{kbps} kbps");
}

#[test]
fn one_pass_produces_both_rates_levels_and_peaks() {
    let d = dir();
    let pcm = fx::tone(1_000.0, 0.5, 1.0, 44_100, 2);
    let path = fx::wav(d.path(), "a.wav", &pcm, 16, false);

    let got = decode(&path, &DecodeOptions::default()).expect("decode");
    assert_eq!(got.mono_44100.len(), 44_100);
    assert_eq!(got.mono_22050.as_ref().map(Vec::len), Some(22_050));
    assert_eq!(got.duration_ms, 1_000);

    // -6.02 dB is 0.5 full scale, and the levels are measured on the source
    // channels, so the downmix cannot have moved them.
    assert!((got.levels.peak_db + 6.02).abs() < 0.1, "{:?}", got.levels);
    assert!(got.levels.true_peak_db >= got.levels.peak_db - 0.01);
    assert_eq!(got.levels.clipped_peaks, 0);
    assert!(got.levels.lufs_integrated.is_finite());

    let peaks = got.peaks.expect("peaks");
    assert_eq!(peaks.sample_rate, ANALYSIS_SAMPLE_RATE);
    assert_eq!(peaks.total_samples, got.mono_44100.len() as u64);
    assert_eq!(peaks.levels.len(), crate::core::peaks::ZOOM_LEVELS.len());
    assert!(peaks.levels[0].max.iter().any(|v| *v > 0.4));
}

#[test]
fn the_downmix_is_the_arithmetic_mean() {
    let d = dir();
    // Opposite constants in the two channels: a mean is exactly silence, and a
    // sum, a max or a power-preserving mix is not.
    let pcm = fx::opposed(0.5, 4_410, 44_100);
    let path = fx::wav(d.path(), "opposed.wav", &pcm, 32, true);

    let got = decode(&path, &DecodeOptions::default()).expect("decode");
    let worst = got.mono_44100.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    assert!(worst < 1e-6, "the downmix left {worst}");
    assert!((got.levels.peak_db + 6.02).abs() < 0.1);
}

#[test]
fn clipping_is_counted_and_silence_stays_finite() {
    let d = dir();
    let hot = fx::wav(d.path(), "hot.wav", &fx::constant(1.0, 1_000, 44_100, 2), 16, false);
    let quiet = fx::wav(
        d.path(),
        "quiet.wav",
        &fx::constant(0.0, 44_100, 44_100, 1),
        16,
        false,
    );

    let hot = decode(&hot, &DecodeOptions::default()).expect("decode");
    assert_eq!(hot.levels.clipped_peaks, 2_000);

    // Digital silence is -inf dB, and the analysis cache is JSON, which cannot
    // carry one: every level has to come back finite.
    let quiet = decode(&quiet, &DecodeOptions::default()).expect("decode");
    assert_eq!(quiet.levels.clipped_peaks, 0);
    assert!(quiet.levels.peak_db.is_finite());
    assert!(quiet.levels.true_peak_db.is_finite());
    assert!(quiet.levels.lufs_integrated.is_finite());
}

#[test]
fn a_48k_source_lands_on_the_analysis_rate() {
    let d = dir();
    let pcm = fx::tone(440.0, 0.5, 1.0, 48_000, 2);
    let path = fx::wav(d.path(), "48k.wav", &pcm, 24, false);

    let got = decode(&path, &DecodeOptions::default()).expect("decode");
    assert_eq!(got.probe.sample_rate, 48_000);
    assert_eq!(got.duration_ms, 1_000);
    let len = got.mono_44100.len() as i64;
    assert!((len - 44_100).abs() <= 2, "{len} samples");
    let key = got.mono_22050.expect("the key rate").len() as i64;
    assert!((key - 22_050).abs() <= 2, "{key} samples");
    // The conversion must carry the signal, not just the right number of
    // samples: a half-amplitude tone has an RMS near 0.354.
    let rms = (got.mono_44100.iter().map(|v| v * v).sum::<f32>() / len as f32).sqrt();
    assert!((rms - 0.354).abs() < 0.02, "rms {rms}");
}

#[test]
fn aiff_decodes_big_endian_samples() {
    let d = dir();
    let pcm = fx::tone(220.0, 0.25, 0.5, 44_100, 2);
    let path = fx::aiff(d.path(), "a.aiff", &pcm);

    let p = probe(&path).expect("probe");
    assert_eq!(p.sample_rate, 44_100);
    assert_eq!(p.channels, 2);

    let got = decode(&path, &DecodeOptions::default()).expect("decode");
    assert_eq!(got.mono_44100.len(), 22_050);
    assert!((got.levels.peak_db + 12.04).abs() < 0.2, "{:?}", got.levels);
}

#[test]
fn the_key_rate_is_skipped_when_it_is_not_wanted() {
    let d = dir();
    let path = fx::wav(
        d.path(),
        "a.wav",
        &fx::tone(440.0, 0.5, 0.25, 44_100, 1),
        16,
        false,
    );
    let opts = DecodeOptions {
        want_key_rate: false,
        want_peaks: false,
        peak_zoom_levels: Vec::new(),
    };
    let got = decode(&path, &opts).expect("decode");
    assert!(got.mono_22050.is_none());
    assert!(got.peaks.is_none());
    assert_eq!(got.mono_44100.len(), 11_025);
}

#[test]
fn resampling_keeps_the_length_and_the_signal() {
    let samples = fx::tone(1_000.0, 0.5, 1.0, 44_100, 1).samples;
    let down = resample(&samples, 44_100, 22_050).expect("down");
    assert_eq!(down.len(), 22_050);
    let rms = (down.iter().map(|v| v * v).sum::<f32>() / down.len() as f32).sqrt();
    assert!((rms - 0.354).abs() < 0.02, "rms {rms}");

    let up = resample(&samples, 44_100, 48_000).expect("up");
    assert_eq!(up.len(), 48_000);

    // Equal rates must not run the filter at all, and an empty clip is empty.
    assert_eq!(resample(&samples, 44_100, 44_100).expect("same"), samples);
    assert!(resample(&[], 44_100, 22_050).expect("empty").is_empty());
    assert!(resample(&samples, 0, 22_050).is_err());
}

#[test]
fn broken_files_fail_without_panicking() {
    let d = dir();
    let empty = d.path().join("empty.wav");
    fs::write(&empty, []).expect("write");
    let text = d.path().join("notes.mp3");
    fs::write(&text, b"this is not audio, it is a text file").expect("write");

    assert!(probe(&empty).is_err());
    assert!(decode(&empty, &DecodeOptions::default()).is_err());
    assert!(audio_region(&empty).is_err());
    assert!(probe(&text).is_err());
    assert!(decode(&text, &DecodeOptions::default()).is_err());

    // A truncated file keeps whatever decoded: refusing it would make a
    // half-copied download unanalysable rather than short.
    let pcm = fx::tone(440.0, 0.5, 1.0, 44_100, 2);
    let cut = fx::truncated_wav(d.path(), "cut.wav", &pcm, 0.4);
    let got = decode(&cut, &DecodeOptions::default()).expect("a short result");
    assert!(got.mono_44100.len() < 44_100 && !got.mono_44100.is_empty());
    assert_eq!(got.duration_ms, (got.mono_44100.len() as i64 * 1000) / 44_100);
}

#[test]
fn a_wrong_extension_still_opens() {
    let d = dir();
    let pcm = fx::tone(440.0, 0.5, 0.25, 44_100, 2);
    let path = fx::wav(d.path(), "mislabelled.mp3", &pcm, 16, false);

    // The extension is a hint. A WAV named .mp3 by a ripper nobody
    // reconfigured is a track, not an error.
    let p = probe(&path).expect("probe");
    assert_eq!(p.codec, "pcm_s16le");
    assert_eq!(decode(&path, &DecodeOptions::default()).expect("decode").mono_44100.len(), 11_025);
}

#[test]
fn a_variable_rate_mp3_is_read_as_one_stream() {
    let d = dir();
    // Enough frames that the demuxer's own duration estimate has something to
    // average over: it reads sixteen frames or sixteen kilobytes, whichever
    // comes first, and gives up on anything shorter.
    let pattern = [128u32, 192, 256, 128, 320, 192];
    let frames: Vec<u32> = pattern.iter().cycle().take(90).copied().collect();
    let path = fx::mp3(d.path(), "vbr.mp3", &frames, 0);

    let p = probe(&path).expect("probe");
    assert_eq!(p.codec, "mp3");
    assert_eq!(p.sample_rate, 44_100);
    assert_eq!(p.channels, 2);
    // Lossy audio has no bit depth, and reporting a plausible 16 would be a
    // fact the file does not contain.
    assert_eq!(p.bit_depth, None);
    assert!(p.duration_ms.unwrap_or(0) > 0);

    // The frames carry no main data, so what comes back is silence — but it
    // comes back: a stream whose frame length changes every frame must not end
    // the pass, which is the failure mode that would drop every VBR MP3 in a
    // library.
    let got = decode(&path, &DecodeOptions::default()).expect("decode");
    assert!(got.duration_ms > 0);
    // Whole MPEG frames, 1,152 samples each, and the reported duration is the
    // count rather than the header's estimate.
    assert_eq!(got.mono_44100.len() % 1_152, 0);
    assert_eq!(
        got.duration_ms,
        (got.mono_44100.len() as i64 * 1_000) / 44_100
    );
}

#[test]
fn the_audio_region_is_unmoved_by_a_tag_rewrite() {
    let d = dir();
    let frames = [128u32, 192, 256, 128, 320, 192, 128, 160];
    let small = fx::mp3(d.path(), "small.mp3", &frames, 1_024);
    let large = fx::mp3(d.path(), "large.mp3", &frames, 8_192);

    let a = audio_region(&small).expect("region");
    let b = audio_region(&large).expect("region");

    // The offsets differ by exactly the tag growth, and everything the content
    // hash reads is identical — which is the whole point: Keystone's own
    // re-tag pass must not wipe its own analysis cache.
    assert_eq!(b.data_offset - a.data_offset, 7_168);
    assert!(a.data_offset >= 1_024);
    assert_eq!(a.data_len, b.data_len);
    assert_eq!(a.codec, b.codec);
    assert_eq!(a.n_frames, b.n_frames);
    assert_eq!(
        head(&small, a.data_offset, 256),
        head(&large, b.data_offset, 256)
    );

    // A trailing ID3v1 tag is not audio either.
    let before = audio_region(&small).expect("region").data_len;
    fx::append_id3v1(&small);
    assert_eq!(audio_region(&small).expect("region").data_len, before);
}

#[test]
fn a_pcm_region_ends_with_its_samples() {
    let d = dir();
    let pcm = fx::tone(440.0, 0.5, 0.5, 44_100, 2);
    let path = fx::wav(d.path(), "a.wav", &pcm, 16, false);

    let region = audio_region(&path).expect("region");
    assert_eq!(region.data_offset, 44);
    assert_eq!(region.data_len, 22_050 * 2 * 2);
    assert_eq!(region.n_frames, 22_050);
}

/// A 30-minute mix, which is the length that makes the memory question real. It
/// is ignored by default because the fixture is 150 MB on disk and the decoded
/// signal is a third of a gigabyte: run it with `cargo test -- --ignored`.
#[test]
#[ignore]
fn a_thirty_minute_mix_returns_a_result() {
    let d = dir();
    let path = fx::long_wav(d.path(), "mix.wav", 1_800, 44_100);
    let opts = DecodeOptions {
        want_key_rate: false,
        ..Default::default()
    };
    let got = decode(&path, &opts).expect("decode");
    assert_eq!(got.duration_ms, 1_800_000);
    assert_eq!(got.mono_44100.len(), 1_800 * 44_100);
    let peaks = got.peaks.expect("peaks");
    assert_eq!(peaks.total_samples, got.mono_44100.len() as u64);
}
