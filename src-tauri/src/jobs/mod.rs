//! Background work that outlives a command: the analysis queue and its writer,
//! the player's read paths, and settings persistence.
//!
//! Nothing here is a Tauri handler and nothing here holds a `State`. Commands
//! validate and delegate into this module, which is what keeps `commands/` thin
//! and lets the queue carry on between calls.

mod admit;
mod dispatch;
pub mod player;
pub mod progress;
pub mod queue;
pub mod settings;
pub mod writer;

pub use progress::QueueCounters;
pub use queue::{AnalysisJob, AnalysisQueue};
