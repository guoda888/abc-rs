use std::fmt::{Debug, Formatter, Result as FmtResult};

use solution::Solution;

#[derive(Clone)]
/// One solution being explored by the hive, plus additional data.
///
/// This implementation was written with the expectation that the
/// [`evaluate_fitness`](trait.Solution.html#tymethod.evaluate_fitness)
/// method may be very expensive, so the `Candidate` struct caches the
/// computed fitness of its solution.
pub struct Candidate<S: Solution> {

    /// Actual candidate solution.
    pub solution: S,

    /// Cached fitness of the solution.
    pub fitness: f64,
}

impl<S: Solution> Candidate<S> {
    pub fn new(solution: S) -> Candidate<S> {
        Candidate {
            fitness: solution.evaluate_fitness(),
            solution: solution,
        }
    }
}

impl<S: Solution + Debug> Debug for Candidate<S> {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(f, "[{}] {:?}", self.fitness, self.solution)
    }
}

pub struct WorkingCandidate<S: Solution> {
    pub candidate: Candidate<S>,
    retries: i32,
}

impl<S: Solution> WorkingCandidate<S> {
    pub fn new(candidate: Candidate<S>, retries: usize) -> WorkingCandidate<S> {
        WorkingCandidate {
            candidate: candidate,
            retries: retries as i32,
        }
    }

    pub fn expired(&self) -> bool {
        self.retries <= 0
    }

    pub fn deplete(&mut self) {
        self.retries -= 1;
    }
}