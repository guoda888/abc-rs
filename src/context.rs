use candidate::Candidate;

/// Context for generating and evaluating solutions.
///
/// The ABC algorithm is abstract enough to work on a variety of problems,
/// some of which may involve a fairly complex interaction with the search
/// space. The `Context` is responsible for maintaining an understanding of
/// that space. That could involve communication or the like. If the problem
/// is straightforward enough not to require this kind of information, a
/// `Context` can be a unit-like struct.
///
/// Note that the `Context` methods all take an immutable `&self` reference.
/// While the algorithm is running, several worker threads will share read-
/// only references to the context. So, if there is any mutable data in the
/// context, it is up to the user to wrap it in a
/// [`Mutex`](https://doc.rust-lang.org/std/sync/struct.Mutex.html) or other
/// locking mechanism. This will allow you to access the fields from multiple
/// threads, without needing a `&mut` reference.
///
/// # Examples
///
/// ```
/// extern crate rand;
/// # extern crate abc; fn main() {
///
/// use abc::{Context, Candidate};
/// use rand::Rng;
///
/// struct Ctx;
///
/// impl Context for Ctx {
///     type Solution = i32;
///
///     fn make(&self) -> i32 {
///         let mut rng = rand::thread_rng();
///         rng.gen_range(0, 100)
///     }
///
///     // Minimize the numerical value.
///     fn evaluate_fitness(&self, solution: &i32) -> f64 {
///         1f64 / *solution as f64
///     }
///
///     fn explore(&self, field: &[Candidate<i32>], n: usize) -> i32 {
///         let mut rng = rand::thread_rng();
///         field[n].solution + rng.gen_range(-10, 10)
///     }
/// }
/// # }
/// ```
pub trait Context : Send + Sync {

    /// Type of solutions generated and evaluated by the ABC.
    ///
    /// For example, a solution for finding the highest point on a 2D map would
    /// be a pair of X and Y coordinates. For more complicated tasks, like
    /// playing a game, this could be a struct with fields for the various
    /// tuning knobs relevant to gameplay.
    type Solution : Clone + Send + Sync + 'static;

    /// Generates a fresh, random solution.
    fn make(&self) -> Self::Solution;

    /// Discovers the fitness of a solution (the algorithm will maximize this).
    ///
    /// Finding an optimal solution depends on having a way to determine
    /// the fitness of one solution compared with another. Because there
    /// are diverse goals for optimization, the user must implement their
    /// own `evaluate_fitness` function.
    ///
    /// The user may wish to use information from the other solutions to
    /// evaluate a given solution. So, rather than simply providing the
    /// solution to be varied, `evaluate_fitness` receives a slice of solution refs
    /// that give information on the existing solutions, and the index of the
    /// solution to be evaluated.
    fn evaluate_fitness(&self, solution: &Self::Solution) -> f64;

    /// Looks "near" an existing solution.
    ///
    /// The user may wish to use information from the other solutions to build
    /// a variant of a given solution. So, rather than simply providing the
    /// solution to be varied, `explore` receives a slice of solution refs
    /// that give information on the existing solutions, and the index of the
    /// solution to be modified.
    fn explore(&self, field: &[Candidate<Self::Solution>], index: usize) -> Self::Solution;
}
