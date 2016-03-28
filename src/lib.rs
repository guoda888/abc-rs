#![crate_name = "abc"]
#![crate_type = "lib"]
#![doc(html_root_url = "https://daviddonna.github.io/abc-rs/")]

#![warn(missing_docs)]

//! Runs Karaboga's Artificial Bee Colony algorithm in parallel.
//!
//! To take advantage of this crate, the user must implement the
//! [`Solution`](trait.Solution.html) trait for a type of their creation.
//! A [`Hive`](struct.Hive.html) of the appropriate type can then be built,
//! which will search the solution space for the fittest candidate.
//!
//! # Examples
//!
//! ```
//! // ABC algorithm with canonical (proportionate) fitness scaling
//! // to minimize the 10-dimensional Rastrigin function.
//!
//! extern crate abc;
//! extern crate rand;
//!
//! use std::f32::consts::PI;
//! use rand::{random, Closed01, thread_rng, Rng};
//! use abc::{Context, Candidate, HiveBuilder};
//!
//! const SIZE: usize = 10;
//!
//! #[derive(Clone, Debug)]
//! struct S([f32;SIZE]);
//!
//! // Not really necessary; we're using this mostly to demonstrate usage.
//! struct SBuilder {
//!     min: f32,
//!     max: f32,
//!     a: f32,
//!     p_min: f32,
//!     p_max: f32,
//! }
//!
//! impl Context for SBuilder {
//!     type Solution = [f32;SIZE];
//!
//!     fn make(&self) -> [f32;SIZE] {
//!         let mut new = [0.0;SIZE];
//!         for i in 0..SIZE {
//!             let Closed01(x) = random::<Closed01<f32>>();
//!             new[i] = (x * (self.max - self.min)) + self.min;
//!         }
//!         new
//!     }
//!
//!     fn evaluate_fitness(&self, solution: &[f32;10]) -> f64 {
//!         let sum = solution.iter()
//!                           .map(|x| x.powf(2.0) - self.a * (*x * 2.0 * PI).cos())
//!                           .fold(0.0, |total, next| total + next);
//!         let rastrigin = ((self.a * SIZE as f32) + sum) as f64;
//!
//!         // Minimize.
//!         if rastrigin >= 0.0 {
//!             1.0 / (1.0 + rastrigin)
//!         } else {
//!             1.0 + rastrigin.abs()
//!         }
//!     }
//!
//!     fn explore(&self, field: &[Candidate<[f32;SIZE]>], index: usize) -> [f32;SIZE] {
//!         // new[i] = current[i] + Φ * (current[i] - other[i]), where:
//!         //      phi_min <= Φ <= phi_max
//!         //      other is a solution, other than current, chosen at random
//!
//!         let ref current = field[index].solution;
//!         let mut new = [0_f32;SIZE];
//!
//!         for i in 0..SIZE {
//!             // Choose a different vector at random.
//!             let mut rng = thread_rng();
//!             let mut index2 = rng.gen_range(0, current.len() - 1);
//!             if index2 >= index { index2 += 1; }
//!             let ref other = field[index2].solution;
//!
//!             let phi = random::<Closed01<f32>>().0 * (self.p_max - self.p_min) + self.p_min;
//!             new[i] = current[i] + (phi * (current[i] - other[i]));
//!         }
//!
//!         new
//!     }
//! }
//!
//! fn main() {
//!     let mut builder = SBuilder {
//!         min: -5.12,
//!         max: 5.12,
//!         a: 10.0,
//!         p_min: -1.0,
//!         p_max: 1.0
//!     };
//!     let hive_builder = HiveBuilder::new(builder, 10);
//!     let hive = hive_builder.build().unwrap();
//!
//!     // Once built, the hive can be run for a number of rounds.
//!     let best_after_10 = hive.run_for_rounds(10).unwrap();
//!
//!     // As long as it's run some rounds at a time, you can keep running it.
//!     let best_after_20 = hive.run_for_rounds(10).unwrap();
//!
//!     // The algorithm doesn't guarantee improvement in any number of rounds,
//!     // but it always keeps its all-time best.
//!     assert!(best_after_20.fitness >= best_after_10.fitness);
//!
//!     // The hive can be consumed to create a Receiver object. This can be
//!     // iterated over indefinitely, and will receive successive improvements
//!     // on the best candidate so far.
//!     let mut current_best_fitness = best_after_20.fitness;
//!     for new_best in hive.stream().iter().take(3) {
//!         assert!(new_best.fitness > current_best_fitness);
//!         current_best_fitness = new_best.fitness;
//!     }
//! }
//! ```

mod result;
mod task;
mod context;
mod candidate;
mod hive;

pub mod scaling;

pub use result::{Error, Result};
pub use context::Context;
pub use candidate::Candidate;
pub use hive::{HiveBuilder, Hive};
