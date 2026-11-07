//! Owns deterministic scenario execution and counterexample reduction.

mod scenario;
mod scheduler;
mod settings;

pub use scenario::{Action, Fault, FaultAt, Latency, Link, SETUPS, Scenario};
pub use scheduler::{Outcome, run, run_with_settings, shrink};
pub use settings::{Mutation, Settings};
