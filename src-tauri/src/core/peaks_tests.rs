use super::*;

fn ramp(n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| (i as f32 / n as f32) * 2.0 - 1.0)
        .collect()
}

#[test]
fn cache_paths_shard_and_stay_inside_the_directory() {
    let p = cache_path(Path::new("/tmp/peaks"), "ab12cd34");
    assert_eq!(p, Path::new("/tmp/peaks/ab/12cd34.peaks"));
    // A short or empty hash must not escape the directory.
    assert!(cache_path(Path::new("/tmp/peaks"), "").starts_with("/tmp/peaks"));
    assert!(cache_path(Path::new("/tmp/peaks"), "a").starts_with("/tmp/peaks"));
}

#[test]
fn the_accumulator_is_fed_in_blocks_and_does_not_care_how_they_are_cut() {
    // The decode loop hands over whatever a packet happens to contain, so the
    // bucket boundaries must not depend on the block size.
    let samples = ramp(10_000);
    let one_block = {
        let mut a = PeakAccumulator::new(&[1024], 44_100);
        a.push(&samples);
        a.finish()
    };
    let many_blocks = {
        let mut a = PeakAccumulator::new(&[1024], 44_100);
        for chunk in samples.chunks(377) {
            a.push(chunk);
        }
        a.finish()
    };
    assert_eq!(one_block.levels[0].min, many_blocks.levels[0].min);
    assert_eq!(one_block.levels[0].max, many_blocks.levels[0].max);
}

#[test]
fn every_bucket_holds_the_extremes_of_its_own_span() {
    let mut a = PeakAccumulator::new(&[4], 44_100);
    a.push(&[0.0, 0.5, -0.25, 0.1, -0.9, 0.2, 0.0, 0.0]);
    let p = a.finish();
    assert_eq!(p.levels[0].max, [0.5, 0.2]);
    assert_eq!(p.levels[0].min, [-0.25, -0.9]);
}

#[test]
fn the_partial_bucket_at_the_end_of_a_track_is_kept() {
    // Dropping it cuts up to 8191 samples off the drawn waveform's end.
    let mut a = PeakAccumulator::new(&[100], 44_100);
    a.push(&[0.3; 150]);
    let p = a.finish();
    assert_eq!(p.levels[0].min.len(), 2);
    assert_eq!(p.levels[0].max[1], 0.3);
}

#[test]
fn every_zoom_level_is_built_from_the_one_decode_pass() {
    let mut a = PeakAccumulator::new(&ZOOM_LEVELS, 44_100);
    a.push(&ramp(44_100));
    let p = a.finish();
    assert_eq!(p.levels.len(), 4);
    for (level, expected) in p.levels.iter().zip(ZOOM_LEVELS) {
        assert_eq!(level.samples_per_bucket, expected);
        let buckets = 44_100_usize.div_ceil(expected as usize);
        assert_eq!(level.min.len(), buckets, "level {expected}");
    }
}

#[test]
fn a_pyramid_round_trips_through_the_cache_file() {
    let dir = tempfile::TempDir::new().unwrap();
    let mut a = PeakAccumulator::new(&[512, 128], 44_100);
    a.push(&ramp(44_100));
    let p = a.finish();
    let expected_min = p.levels[0].min.clone();

    write(dir.path(), "abcd1234", &p).unwrap();
    let got = read(dir.path(), "abcd1234").unwrap().expect("cached");
    assert_eq!(got.content_hash, "abcd1234");
    assert_eq!(got.sample_rate, 44_100);
    assert_eq!(got.levels.len(), 2);
    assert_eq!(got.levels[0].min, expected_min);
    // 44,100 samples at 44.1 kHz is one second.
    assert_eq!(got.duration_ms, 1000);
}

#[test]
fn a_missing_or_corrupt_cache_file_is_a_miss_and_never_an_error() {
    let dir = tempfile::TempDir::new().unwrap();
    assert!(read(dir.path(), "nothing-here").unwrap().is_none());

    let path = cache_path(dir.path(), "ab12");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    // Truncated by a crash, or written by a build with a different layout.
    std::fs::write(&path, b"KSPK\x01\x44").unwrap();
    assert!(read(dir.path(), "ab12").unwrap().is_none());

    std::fs::write(&path, b"not a peaks file at all").unwrap();
    assert!(read(dir.path(), "ab12").unwrap().is_none());
}

#[test]
fn pruning_removes_only_what_nothing_points_at() {
    let dir = tempfile::TempDir::new().unwrap();
    let mut a = PeakAccumulator::new(&[512], 44_100);
    a.push(&ramp(4096));
    let p = a.finish();
    for hash in ["aaaa1111", "bbbb2222", "cccc3333"] {
        write(dir.path(), hash, &p).unwrap();
    }

    let (removed, freed) = prune(dir.path(), &["bbbb2222".into()]).unwrap();
    assert_eq!(removed, 2);
    assert!(freed > 0);
    assert!(read(dir.path(), "bbbb2222").unwrap().is_some());
    assert!(read(dir.path(), "aaaa1111").unwrap().is_none());
}

#[test]
fn pruning_a_directory_that_was_never_created_is_a_no_op() {
    let dir = tempfile::TempDir::new().unwrap();
    assert_eq!(prune(&dir.path().join("never"), &[]).unwrap(), (0, 0));
}

#[test]
fn the_cached_duration_is_the_real_one_and_not_the_padded_bucket_count() {
    // 44,100 samples at 512 per bucket is 87 buckets, which spans 44,544
    // samples. Deriving the duration from the buckets reports 1.010 s for a
    // 1.000 s track, and the player's clock then disagrees with the waveform.
    let dir = tempfile::TempDir::new().unwrap();
    let mut a = PeakAccumulator::new(&[512], 44_100);
    a.push(&ramp(44_100));
    let p = a.finish();
    assert_eq!(p.total_samples, 44_100);
    assert_eq!(p.levels[0].min.len(), 87);

    write(dir.path(), "dddd4444", &p).unwrap();
    assert_eq!(
        read(dir.path(), "dddd4444").unwrap().unwrap().duration_ms,
        1000
    );
}
