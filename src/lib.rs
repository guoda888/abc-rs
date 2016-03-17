//! Runs Karaboga's Artificial Bee Colony algorithm in parallel.
//!
//! To take advantage of this crate, the user must implement the
//! [`Solution`](trait.Solution.html) trait for a type of their creation.
//! A [`Hive`](struct.Hive.html) of the appropriate type can then be built,
//! which will search the solution space for the fittest candidate.

#![warn(missing_docs)]

mod result;
mod task;
mod solution;
mod candidate;
mod hive;

pub mod scaling;

pub use result::{Error, Result};
pub use solution::Solution;
pub use candidate::Candidate;
pub use hive::Hive;