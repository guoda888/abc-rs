extern crate num_cpus;
extern crate itertools;
extern crate rand;
extern crate crossbeam;

use self::rand::Rng;
use self::itertools::Itertools;
use self::crossbeam::{scope, ScopedJoinHandle};

use std::fmt::{Debug, Formatter, Result as FmtResult};
use std::sync::{Mutex, RwLock, LockResult, MutexGuard};

use task::{TaskGenerator, Task};
use candidate::Candidate;
use solution::Solution;
use scaling::{ScalingFunction, proportionate};
use result;

// Completely ignore lock poisoning.
fn force_guard<Guard>(result: LockResult<Guard>) -> Guard {
    match result {
        Ok(x) => x,
        Err(err) => err.into_inner()
    }
}

pub struct Hive<S: Solution> {
    workers: usize,
    observers: usize,
    retries: usize,

    candidates: Vec<RwLock<Candidate<S>>>,
    best: Mutex<Candidate<S>>,
    tasks: Mutex<Option<TaskGenerator>>,

    threads: usize,
    scale: Box<ScalingFunction>,
}

impl<S: Solution + Debug> Debug for Hive<S> {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        for mutex in (&self.candidates).iter() {
            let candidate = mutex.read().unwrap();
            try!(write!(f, "..{:?}..\n", *candidate));
        }
        let best_candidate = self.get().unwrap();
        write!(f, ">>{:?}<<", *best_candidate)
    }
}

impl<S: Solution> Hive<S> {
    pub fn new(workers: usize, observers: usize, retries: usize) -> Hive<S> {
        if workers == 0 {
            panic!("Hive must have at least one worker.");
        }

        let mut raw_candidates = (0..workers)
            .map(|_| Candidate::new(S::make(), retries))
            .collect::<Vec<_>>();

        let best = {
            let best_candidate = raw_candidates.iter()
                .fold1(|best, next| if next.fitness > best.fitness { next } else { best })
                .unwrap();
            Mutex::new(best_candidate.clone())
        };

        let candidates = raw_candidates.drain(..)
            .map(RwLock::new).collect::<Vec<_>>();

        Hive {
            workers: workers,
            observers: observers,
            retries: retries,

            candidates: candidates,
            best: best,
            tasks: Mutex::new(None),

            threads: num_cpus::get(),
            scale: proportionate(),
        }
    }

    pub fn set_threads(mut self, threads: usize) -> Hive<S> {
        self.threads = threads;
        self
    }

    pub fn set_scaling(mut self, scale: Box<ScalingFunction>) -> Hive<S> {
        self.scale = scale;
        self
    }

    fn current_candidates(&self) -> Vec<Candidate<S>> {
        self.candidates.iter()
            .map(|candidate_mutex| {
                let read_guard = force_guard(candidate_mutex.read());
                read_guard.clone()
            })
            .collect()
    }

    fn consider_improvement(&self, candidate: &Candidate<S>) {
        let mut best_guard = force_guard(self.best.lock());
        if candidate.fitness > best_guard.fitness {
            *best_guard = candidate.clone();
        }
    }

    fn work_on(&self, current_candidates: &[Candidate<S>], n: usize) {
        let variant_solution = S::explore(current_candidates, n);
        let variant = Candidate::new(variant_solution, self.retries);

        let mut write_guard = force_guard(self.candidates[n].write());
        if variant.fitness > write_guard.fitness {
            *write_guard = variant;
            self.consider_improvement(&write_guard);
        } else {
            write_guard.deplete();
            // Scouting has been folded into the working process
            if write_guard.expired() {
                let solution = S::make();
                *write_guard = Candidate::new(solution, self.retries);
                self.consider_improvement(&write_guard);
            }
        }
    }

    fn choose(&self, current_candidates: &[Candidate<S>], rng: &mut Rng) -> usize {
        let fitnesses = (self.scale)(current_candidates.iter()
            .map(|candidate| candidate.fitness)
            .collect::<Vec<f64>>());

        let running_totals = fitnesses.iter()
            .scan(0f64, |total, fitness| {
                *total += *fitness;
                Some(*total)
            })
            .collect::<Vec<f64>>();

        let total_fitness = running_totals.last().unwrap();
        let choice_point = rng.next_f64() * total_fitness;

        for (i, total) in running_totals.iter().enumerate() {
            if *total > choice_point {
                return i;
            }
        }
        unreachable!();
    }

    fn execute(&self, task: &Task, rng: &mut Rng) {
        let current_candidates = self.current_candidates();
        let index = match *task {
            Task::Worker(n) => n,
            Task::Observer(_) => self.choose(&current_candidates, rng),
        };
        self.work_on(&current_candidates, index);
    }

    fn run(&self, tasks: TaskGenerator) -> result::Result<()> {
        let mut guard = try!(self.tasks.lock());
        *guard = Some(tasks);
        drop(guard);

        scope(|scope| {
            let mut handles: Vec<ScopedJoinHandle<result::Result<()>>> = Vec::new();

            for _ in 0..self.threads {
                handles.push(scope.spawn(|| {
                    let mut rng = rand::thread_rng();
                    loop {
                        let mut guard = try!(self.tasks.lock());
                        let task = guard.as_mut().and_then(|gen| gen.next());
                        drop(guard);

                        match task {
                            Some(t) => self.execute(&t, &mut rng),
                            None => break
                        };
                    }
                    Ok(())
                }));
            }

            for handle in handles {
                try!(handle.join());
            }

            Ok(())
        })
    }

    pub fn run_for_rounds(&self, rounds: usize) -> result::Result<Candidate<S>> {
        let tasks = TaskGenerator::new(self.workers, self.observers).max_rounds(rounds);
        try!(self.run(tasks));
        let guard = try!(self.get());
        Ok(guard.clone())
    }

    /// Get a guard for the current best solution found by the hive.
    ///
    /// If the hive is running, you should drop the guard returned by this
    /// function as soon as convenient, since the logic of the hive can block
    /// on the availability of the associated mutex. If you plan on performing
    /// expensive computations, you should `drop` the guard as soon as
    /// possible, or acquire and clone it within a small block, like this:
    ///
    /// ```
    /// # extern crate abc; use abc::{Solution, Candidate, Hive};
    /// # #[derive(Clone)] struct X;
    /// # impl Solution for X {
    /// #     fn make() -> X { X }
    /// #     fn evaluate_fitness(&self) -> f64 { 0_f64 }
    /// #     fn explore(field: &[Candidate<X>], n: usize) -> X { X }
    /// # }
    /// # fn main() {
    /// let hive: Hive<X> = Hive::new(5, 5, 5);
    /// let current_best = {
    ///     let guard = hive.get().unwrap();
    ///     guard.clone()
    /// };
    /// # }
    /// ```
    pub fn get(&self) -> result::Result<MutexGuard<Candidate<S>>> {
        self.best.lock().map_err(result::Error::from)
    }

    pub fn stop(&self) -> result::Result<()> {
        self.tasks.lock()
            .map_err(result::Error::from)
            .map(|mut guard| guard.as_mut().map_or((), |t| t.stop()))
    }

    pub fn get_round(&self) -> result::Result<Option<usize>> {
        self.tasks.lock()
            .map_err(result::Error::from)
            .map(|guard| guard.as_ref().map(|tasks| tasks.round))
    }
}

impl<S: Solution> Drop for Hive<S> {
    fn drop(&mut self) {
        self.stop().unwrap()
    }
}