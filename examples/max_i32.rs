extern crate abc;
extern crate rand;

use rand::Rng;

use abc::{Solution, Candidate, HiveBuilder, scaling};

#[derive(Clone, Debug)]
struct Foo(i32);

impl Solution for Foo {
    type Builder = ();

    fn make(_: &mut ()) -> Foo {
        let mut rng = rand::thread_rng();
        let x = rng.gen_range(0, 100);
        Foo(x)
    }

    fn evaluate_fitness(&self) -> f64 {
        let mut x = 0;
        for _ in 0..1_000 {
            x += 1;
        }
        let Foo(y) = *self;
        (x - x) as f64 + y as f64
    }

    fn explore(field: &[Candidate<Foo>], n: usize) -> Foo {
        let mut rng = rand::thread_rng();
        let Foo(x) = field[n].solution;
        Foo(x + rng.gen_range(-10, 10))
    }
}

fn main() {
    let hive = HiveBuilder::<Foo>::new((), 5)
        .set_threads(5)
        .set_scaling(scaling::power_rank(10_f64));
    println!("{:?}", hive.build().unwrap().run_for_rounds(1_000));
}