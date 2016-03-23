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
//! use abc::{Solution, Candidate, HiveBuilder};
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
//! }
//!
//! impl Solution for S {
//!     type Builder = SBuilder;
//!
//!     fn make(builder: &mut SBuilder) -> S {
//!         let mut new = [0.0;SIZE];
//!         for i in 0..SIZE {
//!             let Closed01(x) = random::<Closed01<f32>>();
//!             new[i] = (x * (builder.max - builder.min)) + builder.min;
//!         }
//!         S(new)
//!     }
//!
//!     fn evaluate_fitness(&self) -> f64 {
//!         let a = 10.0;
//!         let sum = self.0.iter()
//!                         .map(|x| x.powf(2.0) - a * (*x * 2.0 * PI).cos())
//!                         .fold(0.0, |total, next| total + next);
//!         let rastrigin = ((a * SIZE as f32) + sum) as f64;
//!
//!         // Minimize.
//!         if rastrigin >= 0.0 {
//!             1.0 / (1.0 + rastrigin)
//!         } else {
//!             1.0 + rastrigin.abs()
//!         }
//!     }
//!
//!     fn explore(field: &[Candidate<S>], index: usize) -> S {
//!         // new[i] = current[i] + Φ * (current[i] - other[i]), where:
//!         //      -1.0 <= Φ <= 1.0
//!         //      other is a solution, other than current, chosen at random
//!
//!         let S(ref current) = field[index].solution;
//!         let mut new = [0_f32;SIZE];
//!
//!         for i in 0..SIZE {
//!             // Choose a different vector at random.
//!             let mut rng = thread_rng();
//!             let mut index2 = rng.gen_range(0, current.len() - 1);
//!             if index2 >= index { index2 += 1; }
//!             let S(ref other) = field[index2].solution;
//!
//!             let phi = random::<Closed01<f32>>().0 * 2.0 - 1.0;
//!             new[i] = current[i] + (phi * (current[i] - other[i]));
//!         }
//!
//!         S(new)
//!     }
//! }
//!
//! fn main() {
//!     let mut builder = SBuilder { min: -5.12, max: 5.12 };
//!     let hive = HiveBuilder::<S>::new(builder, 10);
//!     println!("{:?}", hive.build().unwrap().run_for_rounds(100).unwrap());
//! }
//! ```

mod result;
mod task;
mod solution;
mod candidate;
mod hive;

pub mod scaling;

pub use result::{Error, Result};
pub use solution::Solution;
pub use candidate::Candidate;
pub use hive::{HiveBuilder, Hive};
