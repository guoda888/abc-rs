use candidate::Candidate;

/// Candidate solution for an optimization problem.
///
/// The ABC algorithm is abstract enough to work on a variety of
/// problems.
///
/// # Examples
///
/// ```
/// extern crate rand;
/// # extern crate abc; fn main() {
///
/// use abc::{Solution, Candidate};
/// use rand::Rng;
///
/// // Because i32 and abc::Solution are both defined elsewhere,
/// // we cannot implement Solution for i32 directly. So, we use
/// // a struct as a thin wrapper.
/// #[derive(Clone)]
/// struct Number(i32);
///
/// impl Solution for Number {
///     type Builder = ();
///
///     fn make(_: &mut ()) -> Number {
///         let mut rng = rand::thread_rng();
///         let x = rng.gen_range(0, 100);
///         Number(x)
///     }
///
///     // Minimize the numerical value.
///     fn evaluate_fitness(&self) -> f64 {
///         let Number(x) = *self;
///         1f64 / x as f64
///     }
///
///     fn explore(field: &[Candidate<Number>], n: usize) -> Number {
///         let mut rng = rand::thread_rng();
///         let Number(x) = field[n].solution;
///         Number(x + rng.gen_range(-10, 10))
///     }
/// }
/// # }
/// ```
pub trait Solution : Clone + Send + Sync + 'static {

    /// Factory that can be used to generate new solutions.
    ///
    /// In cases where the user wishes to generate new solutions based
    /// on some set of parameters, or deterministically, the `Builder`
    /// can hold necessary data.
    ///
    /// If no builder is necessary, this type can be `()`.
    type Builder : Send;

    /// Generate a fresh, random solution.
    ///
    /// The name of this method has been chosen to avoid colliding with
    /// a presumed `Self::new(...)` method.
    fn make(builder: &mut Self::Builder) -> Self;

    /// Discover the fitness of a solution (goal is to maximize).
    ///
    /// Finding an optimal solution depends on having a way to determine
    /// the fitness of one solution compared with another. Because there
    /// are diverse goals for optimization, the user must implement their
    /// own `evaluate_fitness` function.
    fn evaluate_fitness(&self) -> f64;

    /// Look "near" an existing solution.
    ///
    /// The user may wish to use information from the other solutions to
    /// build a variant of a given solution. So, rather than simply
    /// providing the solution to be varied, `explore` receives a vector
    /// of [Candidates](struct.Candidate.html) that give information on the existing
    /// solutions, and the index of the solution to be modified.
    fn explore(solutions: &[Candidate<Self>], index: usize) -> Self;
}
