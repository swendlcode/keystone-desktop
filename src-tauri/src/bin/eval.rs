//! The accuracy harness as a command, so it can be run by hand and its report
//! read in full rather than through a test runner's capture.
//!
//! **Dev-only.** `required-features = ["eval"]` in `Cargo.toml` keeps it out of
//! every build that does not ask for it, and the bundle does not ask for it.
//!
//!   cargo run --features eval --bin keystone-eval
//!   cargo run --features eval --bin keystone-eval -- <giantsteps dir>
//!   cargo run --features eval --bin keystone-eval -- --track <audio file>
//!   cargo run --features eval --bin keystone-eval -- --energy-features <path list>
//!   cargo run --features eval --bin keystone-eval -- --key-table [threads] < paths
//!
//! With no argument it runs the synthetic corpus, which needs no files. With a
//! directory it runs the real gate from ENGINE.md 7.1 over the audio in that
//! directory. With `--track` it runs the whole engine over one file and prints
//! every field, which is the only one of the corpus-wide modes that would notice a stage
//! returning a plausible wrong answer on a real record. With `--energy-features`
//! it prints the seven energy features for every file in a list, which is the
//! corpus pass the energy quantile ladder is calibrated from.

use keystone_lib::eval::{self, energy_features, key_table, one_track, report, synthetic};

fn main() {
    let first = std::env::args().nth(1);
    if first.as_deref() == Some("--track") {
        one_file(std::env::args().nth(2));
        return;
    }
    if first.as_deref() == Some("--energy-features") {
        energy_rows(std::env::args().nth(2));
        return;
    }
    if first.as_deref() == Some("--key-table") {
        key_table(std::env::args().nth(2));
        return;
    }
    match first {
        None => match synthetic::run() {
            Ok(r) => print!("{}", report::synthetic(&r)),
            Err(e) => {
                eprintln!("harness failed: {e}");
                std::process::exit(1);
            }
        },
        Some(dir) => {
            let opts = eval::HarnessOptions::default();
            match eval::run(std::path::Path::new(&dir), &opts) {
                Ok(r) => print!("{}", report::corpus_run("GiantSteps+", &r)),
                Err(e) => {
                    eprintln!("{e}");
                    std::process::exit(1);
                }
            }
        }
    }
}

fn energy_rows(list: Option<String>) {
    let Some(list) = list else {
        eprintln!("--energy-features needs a path to a file holding one audio path per line");
        std::process::exit(2);
    };
    let threads = std::env::args()
        .nth(3)
        .and_then(|a| a.parse().ok())
        .unwrap_or(energy_features::DEFAULT_THREADS);
    if let Err(e) = energy_features::run(std::path::Path::new(&list), threads) {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

fn one_file(path: Option<String>) {
    let Some(path) = path else {
        eprintln!("--track needs a path to an audio file");
        std::process::exit(2);
    };
    let path = std::path::Path::new(&path);
    match one_track::run(path) {
        Ok(result) => print!("{}", one_track::report(path, &result)),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}

/// One TSV row of raw key evidence per path on stdin.
///
/// Paths on stdin rather than as arguments because the corpus is built by a
/// `find | grep` sweep and a thousand of them do not fit in an argument list.
fn key_table(threads: Option<String>) {
    use std::io::{BufRead, Write};
    let threads = threads.and_then(|t| t.parse::<usize>().ok()).unwrap_or(6);
    let paths: Vec<std::path::PathBuf> = std::io::stdin()
        .lock()
        .lines()
        .map_while(std::result::Result::ok)
        .map(|l| l.trim_end().to_string())
        .filter(|l| !l.is_empty())
        .map(std::path::PathBuf::from)
        .collect();
    let out = std::io::stdout();
    let mut out = std::io::BufWriter::new(out.lock());
    let _ = writeln!(out, "{}", key_table::HEADER);
    for line in key_table::run(&paths, threads) {
        let _ = writeln!(out, "{line}");
    }
}
