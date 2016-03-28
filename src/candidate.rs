use std::fmt::{Debug, Formatter, Result as FmtResult};

#[derive(Clone)]
/// One solution being explored by the hive, plus additional data.
///
/// This implementation was written with the expectation that the
/// [`evaluate_fitness`](trait.Solution.html#tymethod.evaluate_fitness)
/// method may be very expensive, so the `Candidate` struct caches the
/// computed fitness of its solution.
pub struct Candidate<S: Clone + Send + Sync + 'static> {
    /// Actual candidate solution.
    pub solution: S,

    /// Cached fitness of the solution.
    pub fitness: f64,
}

impl<S: Clone + Send + Sync + 'static> Candidate<S> {
    /// Wrap a solution with its cached fitness.
    pub fn new(solution: S, fitness: f64) -> Candidate<S> {
        Candidate {
            solution: solution,
            fitness: fitness,
        }
    }
}

impl<S: Clone + Send + Sync + 'static> Debug for Candidate<S>
    where S: Debug
{
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(f, "[{}] {:?}", self.fitness, self.solution)
    }
}

pub struct WorkingCandidate<S: Clone + Send + Sync + 'static> {
    pub candidate: Candidate<S>,
    retries: i32,
}

impl<S: Clone + Send + Sync + 'static> WorkingCandidate<S> {
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
