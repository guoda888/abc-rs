extern crate num_cpus;
extern crate itertools;
extern crate rand;
extern crate crossbeam;

use self::rand::Rng;
use self::itertools::Itertools;

use std::thread::spawn;
use std::fmt::{Debug, Formatter, Result as FmtResult};
use std::sync::{Mutex, RwLock, LockResult, MutexGuard};
use std::sync::mpsc::{Sender, Receiver, channel};

use task::{TaskGenerator, Task};
use candidate::Candidate;
use solution::Solution;
use scaling::{ScalingFunction, proportionate};

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
        let best_candidate = self.get();
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
                let read_lock = force_guard(candidate_mutex.read());
                read_lock.clone()
            })
            .collect()
    }

    fn consider_improvement(&self, candidate: &Candidate<S>) {
        let mut best_lock = force_guard(self.best.lock());
        if candidate.fitness > best_lock.fitness {
            *best_lock = candidate.clone();
        }
    }

    fn work_on(&self, current_candidates: &[Candidate<S>], n: usize) {
        let variant_solution = S::explore(current_candidates, n);
        let variant = Candidate::new(variant_solution, self.retries);

        let mut write_lock = force_guard(self.candidates[n].write());
        if variant.fitness > write_lock.fitness {
            *write_lock = variant;
            self.consider_improvement(&write_lock);
        } else {
            write_lock.deplete();
            // Scouting has been folded into the working process
            if write_lock.expired() {
                let solution = S::make();
                *write_lock = Candidate::new(solution, self.retries);
                self.consider_improvement(&write_lock);
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

    fn run(&self, tasks: TaskGenerator) {
        let mut tasks_lock = self.tasks.lock().unwrap();
        *tasks_lock = Some(tasks);
        drop(tasks_lock);
        crossbeam::scope(|scope| {
            for _ in 0..self.threads {
                scope.spawn(|| {
                    let mut rng = rand::thread_rng();
                    loop {
                        let mut tasks_lock = self.tasks.lock().unwrap();
                        let task = tasks_lock.as_mut().and_then(|gen| gen.next());
                        drop(tasks_lock);

                        match task {
                            Some(t) => self.execute(&t, &mut rng),
                            None => break
                        };
                    }
                });
            }
        });
    }

    pub fn run_for_rounds(&self, rounds: usize) -> Candidate<S> {
        let tasks = TaskGenerator::new(self.workers, self.observers).max_rounds(rounds);
        self.run(tasks);
        self.get().clone()
    }

    pub fn stream(&self) -> Receiver<Candidate<S>> {
        let (tx, rx) = channel();
        panic!("Not implemented!");
        rx
    }

    /// Get a guard for the current best solution found by the hive.
    ///
    /// If the hive is running, you should drop the guard returned by this
    /// function as soon as convenient, since the logic of the hive can block
    /// on the availability of the associated mutex. If you plan on performing
    /// expensive computations, you should clone the guard inside a block:
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
    ///     let lock = hive.get();
    ///     lock.clone()
    /// };
    /// # }
    /// ```
    pub fn get(&self) -> MutexGuard<Candidate<S>> {
        self.best.lock().unwrap()
    }

    pub fn stop(&self) {
        let mut tasks_lock = self.tasks.lock().unwrap();
        tasks_lock.as_mut().map_or((), |t| t.stop())
    }

    pub fn get_round(&self) -> Option<usize> {
        let tasks_lock = self.tasks.lock().unwrap();
        tasks_lock.as_ref().map(|tasks| tasks.round)
    }
}

impl<S: Solution> Drop for Hive<S> {
    fn drop(&mut self) {
        self.stop();
    }
}