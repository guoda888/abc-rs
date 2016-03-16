// #![warn(missing_docs)]

mod task;
mod solution;
mod candidate;
mod hive;

#[allow(unused_attributes)]
pub mod scaling;

pub use solution::Solution;
pub use candidate::Candidate;
pub use hive::Hive;