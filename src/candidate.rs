use std::fmt::{Debug, Formatter, Result as FmtResult};

use solution::Solution;

/// One solution being explored by the hive, plus additional data.
#[derive(Clone)]
pub struct Candidate<S: Solution> {

    /// Actual candidate solution.
    pub solution: S,

    /// Cached fitness of the solution.
    pub fitness: f64,

    retries: i32,
}

impl<S: Solution> Candidate<S> {
    pub fn new(solution: S, retries: usize) -> Candidate<S> {
        Candidate {
            fitness: solution.evaluate_fitness(),
            solution: solution,
            retries: retries as i32,
        }
    }

    pub fn expired(&self) -> bool {
        self.retries <= 0
    }

    pub fn deplete(&mut self) {
        self.retries -= 1;
    }

    pub fn expire(&mut self) {
        self.retries = 0;
    }
}

impl<S: Solution + Debug> Debug for Candidate<S> {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(f, "[{}:{}] {:?}", self.fitness, self.retries, self.solution)
    }
}