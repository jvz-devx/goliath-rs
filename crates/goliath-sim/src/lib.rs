//! Owns deterministic scenario execution and counterexample reduction.

mod scenario;
mod scheduler;

pub use scenario::{Action, Fault, FaultAt, Latency, Link, SETUPS, Scenario};
pub use scheduler::{Outcome, run, shrink};
