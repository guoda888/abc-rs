// #![warn(missing_docs)]

mod result;
mod task;
mod solution;
mod candidate;
mod hive;

#[allow(unused_attributes)]
pub mod scaling;

pub use result::{Error, Result};
pub use solution::Solution;
pub use candidate::Candidate;
pub use hive::Hive;