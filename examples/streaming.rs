extern crate abc;
extern crate rand;

use rand::{Rng, thread_rng};

use abc::{Context, Candidate, HiveBuilder, scaling};

#[derive(Clone, Debug)]
struct Foo;

impl Context for Foo {
    type Solution = i32;

    fn make(&self) -> i32 {
        thread_rng().gen_range(0, 100)
    }

    fn evaluate_fitness(&self, solution: &Self::Solution) -> f64 {
        let mut x = 0;
        for _ in 0..1_000 {
            x += 1;
        }
        (x - x) as f64 + *solution as f64
    }

    fn explore(&self, field: &[Candidate<i32>], n: usize) -> i32 {
        field[n].solution + thread_rng().gen_range(-10, 10)
    }
}

fn main() {
    let hive = HiveBuilder::<Foo>::new(Foo, 5)
        .set_threads(5)
        .set_observers(4)
        .set_scaling(scaling::power_rank(10_f64));
    for candidate in hive.build()
                         .unwrap()
                         .stream()
                         .iter()
                         .skip_while(|c| c.fitness < 200_f64)
                         .take(5) {
        println!("{:?}", candidate);
    }
}