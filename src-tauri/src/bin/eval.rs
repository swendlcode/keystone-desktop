//! The accuracy harness as a command, so it can be run by hand and its report
//! read in full rather than through a test runner's capture.
//!
//! **Dev-only.** `required-features = ["eval"]` in `Cargo.toml` keeps it out of
//! every build that does not ask for it, and the bundle does not ask for it.
//!
//!   cargo run --features eval --bin keystone-eval
//!   cargo run --features eval --bin keystone-eval -- <giantsteps dir>
//!   cargo run --features eval --bin keystone-eval -- --track <audio file>
//!
//! With no argument it runs the synthetic corpus, which needs no files. With a
//! directory it runs the real gate from ENGINE.md 7.1 over the audio in that
//! directory. With `--track` it runs the whole engine over one file and prints
//! every field, which is the only one of the three that would notice a stage
//! returning a plausible wrong answer on a real record.

use keystone_lib::eval::{self, one_track, report, synthetic};

fn main() {
    let first = std::env::args().nth(1);
    if first.as_deref() == Some("--track") {
        one_file(std::env::args().nth(2));
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
