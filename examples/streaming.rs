extern crate abc;
extern crate rand;

use rand::Rng;

use abc::{Solution, Candidate, Hive, scaling};

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
    let hive: Hive<Foo> = Hive::new((), 5)
        .set_threads(5)
        .set_observers(4)
        .set_scaling(scaling::power_rank(10f64));
    for candidate in hive.swarm()
                         .unwrap()
                         .stream()
                         .iter()
                         .skip_while(|c| c.fitness < 200_f64)
                         .take(5) {
        println!("{:?}", candidate);
    }
}