//! The accuracy harness as a command, so it can be run by hand and its report
//! read in full rather than through a test runner's capture.
//!
//! **Dev-only.** `required-features = ["eval"]` in `Cargo.toml` keeps it out of
//! every build that does not ask for it, and the bundle does not ask for it.
//!
//!   cargo run --features eval --bin keystone-eval
//!   cargo run --features eval --bin keystone-eval -- <giantsteps dir>
//!
//! With no argument it runs the synthetic corpus, which needs no files. With a
//! directory it runs the real gate from ENGINE.md 7.1 — which is blocked on S1
//! decode and says so rather than pretending.

use keystone_lib::eval::{self, report, synthetic};

fn main() {
    let corpus = std::env::args().nth(1);
    match corpus {
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
                Ok(r) => {
                    print!("{}", report::key_score("GiantSteps+", &r.key));
                    println!("{} tracks in {} ms", r.tracks_scored, r.elapsed_ms);
                    println!("\n{}", report::CAVEAT);
                }
                Err(e) => {
                    eprintln!("{e}");
                    std::process::exit(1);
                }
            }
        }
    }
}
