//! The inputs that must not panic, because a panic on an analysis worker takes
//! the queue down and the user's library with it.
//!
//! Silence, a fifth of a second, a downmixed stereo source, 96 kHz and a
//! thirty-minute DJ mix. Each one goes through both paths that can take an
//! arbitrary buffer today: S2-S3-S4 (tuning, chroma, key) and S6-S7-S8 (onsets,
//! tempo, grid).
//!
//! **S9-S11 are not reachable from audio yet.** The time-domain front end they
//! share — `analysis::frontend`'s STFT and HPSS — is a stub, so structure,
//! energy and cues can only be driven from a spectral-domain fixture. Their edge
//! cases are therefore *not* covered here and must be re-run against these same
//! inputs when S1 and that front end land. Nor is any of this a decode test: no
//! file is read, so a truncated MP3, a zero-byte file and a file whose header
//! lies about its length remain unmeasured.

#![cfg(feature = "eval")]

use std::time::Instant;

use keystone_lib::analysis::camelot::Camelot;
use keystone_lib::analysis::rhythm;
use keystone_lib::analysis::spectral::{test_signals as sig, test_unpitched as unp};
use keystone_lib::eval::corpus;
use keystone_lib::eval::key_path;
use keystone_lib::models::KeyMode;

/// Runs both paths and checks the invariants every result owes whatever the
/// input was: a parseable Camelot code, a confidence inside 0..1, a root inside
/// 0..11, and a tempo that is either a positive number or no number.
fn both_paths(label: &str, samples: &[f32], sample_rate: u32) {
    let started = Instant::now();
    let run = key_path::run(samples, sample_rate, None)
        .unwrap_or_else(|e| panic!("{label}: key path failed: {e}"));
    let key_ms = started.elapsed().as_millis();
    let r = &run.detection.result;
    assert!(Camelot::parse(&r.camelot).is_some(), "{label}: unparseable camelot {}", r.camelot);
    assert!(r.root < 12, "{label}: root {}", r.root);
    assert!((0.0..=1.0).contains(&r.confidence), "{label}: confidence {}", r.confidence);
    assert!(r.tuning_cents.is_finite(), "{label}: tuning {} cents", r.tuning_cents);

    let started = Instant::now();
    let analysis = rhythm::analyze(samples, sample_rate, &[])
        .unwrap_or_else(|e| panic!("{label}: rhythm path failed: {e}"));
    let rhythm_ms = started.elapsed().as_millis();
    let t = &analysis.tempo;
    assert!(t.raw_bpm.is_finite() && t.raw_bpm >= 0.0, "{label}: bpm {}", t.raw_bpm);
    assert!((0.0..=1.0).contains(&t.confidence), "{label}: tempo confidence {}", t.confidence);
    assert!(
        analysis.grid.coverage.is_finite() && (0.0..=1.0).contains(&analysis.grid.coverage),
        "{label}: coverage {}",
        analysis.grid.coverage
    );
    // A grid may be empty, but it may never be unsorted or hold a non-finite
    // time: every stage downstream indexes into it.
    let beats = &analysis.grid.beats_ms;
    assert!(beats.iter().all(|t| t.is_finite()), "{label}: non-finite beat time");
    assert!(beats.windows(2).all(|w| w[1] >= w[0]), "{label}: beat times out of order");

    println!(
        "{label:34} {:>9} samples @ {sample_rate:6} Hz -> {} ({:.2}) / {:.2} BPM \
         ({:.2}) | key {key_ms} ms, rhythm {rhythm_ms} ms",
        samples.len(),
        r.camelot,
        r.confidence,
        t.raw_bpm,
        t.confidence
    );
}

#[test]
fn silence_in_every_length_is_not_a_panic() {
    for seconds in [0.0f32, 0.05, 0.2, 1.0, 16.0] {
        let samples = unp::silence(44_100, seconds);
        both_paths(&format!("silence {seconds} s"), &samples, 44_100);
        let run = key_path::run(&samples, 44_100, None).expect("key path");
        assert!(run.detection.ambiguous, "silence was given a key");
        assert_eq!(run.detection.result.confidence, 0.0);
    }
    // A zero-length buffer is what a decode that produced nothing hands the
    // worker, and a zero sample rate is what a header that lied hands it.
    both_paths("empty buffer", &[], 44_100);
}

#[test]
fn a_fifth_of_a_second_is_too_short_to_answer_and_says_so() {
    // Below S-KEY's 3 s minimum and below one 15 s chunk, so the key stage falls
    // back to the whole-track PCP over about four chroma frames. An answer from
    // that is not an answer, and the tempo of a fifth of a second does not
    // exist at all.
    for tonic in [0u8, 7] {
        let samples = sig::progression(&sig::major_pop(), tonic as i32, 44_100, 0.2, 0.0);
        both_paths(&format!("0.2 s progression, tonic {tonic}"), &samples, 44_100);
        let run = key_path::run(&samples, 44_100, None).expect("key path");
        assert!(
            run.detection.ambiguous,
            "a fifth of a second was answered as {} at confidence {:.2}",
            run.detection.result.camelot,
            run.detection.result.confidence
        );
        assert_eq!(run.detection.result.alt_root, None, "0.2 s offered an alternative key");
    }
    both_paths("0.2 s percussion", &unp::percussion(44_100, 0.2), 44_100);
}

#[test]
fn a_downmixed_stereo_source_answers_the_same_as_the_mono_original() {
    // Every stage after S1 sees one channel, so this is the only place the
    // arithmetic-mean downmix rule can be checked until S1 exists. The right
    // channel is the same performance 3 dB down and 5 ms late, which is what a
    // stereo mix is and what a sum-instead-of-mean or left-only downmix would
    // get visibly wrong.
    let rate = sig::TEST_RATE;
    let mono = sig::progression(&sig::minor_iv(), 9, rate, 12.0, 0.0);
    let delay = (rate as f32 * 0.005) as usize;
    let mut right = vec![0.0f32; mono.len()];
    right[delay..].copy_from_slice(&mono[..mono.len() - delay]);
    right.iter_mut().for_each(|s| *s *= 0.707);

    let stereo = corpus::interleave(&mono, &right);
    assert_eq!(stereo.len(), mono.len() * 2);
    let downmixed = corpus::downmix_stereo(&stereo);
    assert_eq!(downmixed.len(), mono.len());

    both_paths("stereo, downmixed", &downmixed, rate);
    let from_mono = key_path::run(&mono, rate, None).expect("mono");
    let from_stereo = key_path::run(&downmixed, rate, None).expect("downmixed");
    assert_eq!(
        (from_stereo.detection.result.root, from_stereo.detection.result.mode),
        (from_mono.detection.result.root, from_mono.detection.result.mode),
        "downmixing changed the key from {} to {}",
        from_mono.detection.result.camelot,
        from_stereo.detection.result.camelot
    );
    assert_eq!((from_mono.detection.result.root, from_mono.detection.result.mode), (9, KeyMode::Minor));
    // An odd-length interleaved buffer is a truncated frame, not a panic.
    let odd = corpus::downmix_stereo(&stereo[..stereo.len() - 1]);
    assert_eq!(odd.len(), mono.len());
}

#[test]
fn ninety_six_kilohertz_is_analysed_at_its_own_rate() {
    // Nothing in either path may assume 44.1 kHz: the onset hop is derived from
    // the rate so that the 100 fps contract every frame index downstream assumes
    // still holds, and the chroma ceiling is clamped against Nyquist.
    let samples = sig::progression(&sig::major_cadence(), 2, 96_000, 8.0, 0.0);
    both_paths("96 kHz progression", &samples, 96_000);
    let run = key_path::run(&samples, 96_000, None).expect("key path");
    assert_eq!(
        (run.detection.result.root, run.detection.result.mode),
        (2, KeyMode::Major),
        "D major at 96 kHz came back as {}",
        run.detection.result.camelot
    );

    let drums = unp::percussion(96_000, 20.0);
    let analysis = rhythm::analyze(&drums, 96_000, &[]).expect("rhythm");
    println!(
        "96 kHz percussion -> {:.2} BPM at {:.2} fps",
        analysis.tempo.raw_bpm, analysis.curves.fps
    );
    assert!(
        (analysis.curves.fps - 100.0).abs() < 1.0,
        "the novelty curve ran at {:.2} fps at 96 kHz",
        analysis.curves.fps
    );
}

#[test]
fn a_thirty_minute_dj_mix_completes_without_a_panic_or_an_allocation_blowup() {
    // Thirty minutes is the shape of the input, not a performance benchmark:
    // the per-track budget in ENGINE.md 2 is measured on five-minute tracks on
    // an M2 Max in a release build, and this is a debug build. What is being
    // checked is that nothing indexes off the end of a long buffer, that the
    // beat grid stays sorted over tens of thousands of beats, and that no stage
    // tries to hold a quadratic-sized intermediate.
    let rate = sig::TEST_RATE;
    let block_seconds = 30.0f32;
    let blocks: Vec<Vec<f32>> = vec![
        sig::progression(&sig::major_pop(), 0, rate, block_seconds, 0.0),
        sig::progression(&sig::minor_iv(), 9, rate, block_seconds, 0.0),
        unp::percussion(rate, block_seconds),
    ];
    let wanted = (rate as f32 * 30.0 * 60.0) as usize;
    let mut mix = Vec::with_capacity(wanted + rate as usize);
    while mix.len() < wanted {
        mix.extend_from_slice(&blocks[(mix.len() / (rate as usize * 30)) % blocks.len()]);
    }
    mix.truncate(wanted);
    let minutes = mix.len() as f32 / rate as f32 / 60.0;
    assert!((minutes - 30.0).abs() < 0.1, "built {minutes:.2} minutes");

    both_paths("30-minute mix", &mix, rate);
    let run = key_path::run(&mix, rate, None).expect("key path");
    // A mix is not in one key, and the per-chunk votes are what say so. One vote
    // per 15 s over thirty minutes is 120 of them.
    let chunks = run.detection.result.chunk_keys.len();
    println!("30-minute mix: {chunks} chunk votes, agreement {:.2}", run.detection.chunk_agreement);
    assert!(chunks > 100, "only {chunks} chunk votes over thirty minutes");
    assert!(run.detection.chunk_agreement < 1.0, "a mix of three keys agreed with itself");
    assert!(
        run.detection.result.chunk_keys.windows(2).all(|w| w[1].start_ms > w[0].start_ms),
        "chunk times out of order"
    );
}
