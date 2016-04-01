
//! Manipulates the probabilities of working on different solutions.
//!
//! A portion of the bees in an artificial bee colony are tasked with observing
//! the dedicated workers, and doing extra work on promising solutions. To
//! enable this, the solutions' fitnesses are gathered as a `Vec<f64>`. A
//! [`ScalingFunction`](type.ScalingFunction.html) is then run on the fitnesses to
//! get weighting factors, and a solution is chosen with likelihood
//! proportionate to its scaled fitness. This is expressed as:
//!
//! <center>P(*i*) = *scaled*<sub>*i*</sub>
//! / ∑<sub>*j* = 1 … N</sub> *scaled*<sub>*j*</sub></center>
//!
//! By default, [proportionate](fn.proportionate.html) scaling is used.
//!
//! # Examples
//!
//! Several constructors for scaling functions are available in this module.
//! However, users may also implement custom scaling functions. These can,
//! for example, exaggerate differences in fitness:
//!
//! ```
//! # extern crate abc; fn main() {
//! Box::new(move |fitnesses: Vec<f64>| {
//!     // Square the fitnesses.
//!     fitnesses.iter().map(|fitness| fitness.powf(2_f64)).collect::<Vec<_>>()
//! });
//! # }
//! ```
//!
//! If you have a large number of active solutions, and don't want to replicate
//! the fitnesses vector, you can mutate and return the same vector. Since the
//! actual storage portion of a `Vec` is is heap-allocated, the scaling function
//! should be reasonably well-behaved with respect to memory.

/// Transform a set of fitnesses into weights for observers' random choices.
pub type ScalingFunction = Fn(Vec<f64>) -> Vec<f64> + Send + Sync + 'static;

/// Chooses solutions in direct proportion to their fitness.
///
/// scaled<sub>*i*</sub> = fitness<sub>*i*</sub>
pub fn proportionate() -> Box<ScalingFunction> {
    Box::new(move |fitnesses: Vec<f64>| fitnesses)
}

/// Chooses more fit solutions exponentially more often.
///
/// scaled<sub>*i*</sub> = fitness<sub>*i*</sub><sup>*k*</sup>
pub fn power(k: f64) -> Box<ScalingFunction> {
    Box::new(move |mut fitnesses: Vec<f64>| {
        for f in &mut fitnesses {
            *f = f.powf(k);
        }
        fitnesses
    })
}

/// Chooses solutions according to their rank.
///
/// Rather than use the fitness directly, this formula ranks the N solutions
/// 1 to N, in ascending order of fitness, then chooses in proportion to the
/// value of the rank. So, the solution with rank 6 will be chosen twice as
/// often as the solution with rank 3, and three times as often as the solution
/// with rank 2.
///
/// scaled<sub>*i*</sub> = rank<sub>*i*</sub>
pub fn rank() -> Box<ScalingFunction> {
    // power_rank is be implemented on its own because composing scaling
    // functions involves allocating extra vectors, which we'd like to avoid.
    power_rank(1_f64)
}

/// Chooses solutions according to their rank, raised to a certain power.
///
/// This scaling formula was proposed by Yudong Zhang et al for the
/// Fitness-Scaling Chaotic ABC in the 2013 volume of *Mathematical Problems
/// in Engineering*. Conceptually, it composes the [power](fn.power.html) and
/// [rank](fn.rank.html) scaling techniques.
///
/// scaled<sub>*i*</sub> = rank<sub>*i*</sub><sup>*k*</sup>
///
/// As with rank scaling, rank<sub>*i*</sub> starts with 1 for the least fit,
/// and continues up to N for the most fit.
pub fn power_rank(k: f64) -> Box<ScalingFunction> {
    Box::new(move |fitnesses: Vec<f64>| {
        // Pair each fitness with its index, so that we can remember which goes
        // where after sorting.
        let mut with_indices = fitnesses.iter().enumerate().collect::<Vec<_>>();

        // Sort by fitness, ascending. After this, we can ignore fitness.
        with_indices.sort_by(|&(_, f1), &(_, f2)| f1.partial_cmp(f2).unwrap());

        // The rank of solution i now corresponds to the index in with_indices
        // of (i, fitness_i). But we want the original index to be the index,
        // and the rank to be used to generate the value.

        // Create a blank (not empty) vector, so that we can use random access
        // to sort by original index.
        let mut ranks = vec![0_f64;with_indices.len()];
        for (rank_minus_one, &(index, _)) in with_indices.iter().enumerate() {
            ranks[index] = ((rank_minus_one + 1) as f64).powf(k);
        }
        ranks
    })
}
