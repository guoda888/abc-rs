extern crate num_cpus;
extern crate itertools;
extern crate rand;
extern crate crossbeam;

use self::rand::{thread_rng, Rng};
use self::itertools::Itertools;
use self::crossbeam::{scope, ScopedJoinHandle};

use std::fmt::{Debug, Formatter, Result as FmtResult};
use std::sync::{Mutex, RwLock, MutexGuard};
use std::sync::mpsc::{Sender, Receiver, channel};
use std::thread::spawn;

use task::{TaskGenerator, Task};
use candidate::{WorkingCandidate, Candidate};
use solution::Solution;
use scaling::{ScalingFunction, proportionate};
use result::{Result as AbcResult, Error as AbcError};

/// Manages the running of the ABC algorithm.
pub struct Hive<S: Solution> {
    workers: usize,
    observers: usize,
    retries: usize,

    builder: Mutex<S::Builder>,
    working: Vec<RwLock<WorkingCandidate<S>>>,
    best: Mutex<Candidate<S>>,
    tasks: Mutex<Option<TaskGenerator>>,
    streaming: Option<Mutex<Sender<Candidate<S>>>>,

    threads: usize,
    scale: Box<ScalingFunction>,
}

impl<S: Solution + Debug> Debug for Hive<S> {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        for mutex in (&self.working).iter() {
            let working = mutex.read().unwrap();
            try!(write!(f, "..{:?}..\n", working.candidate));
        }
        let best_candidate = self.get().unwrap();
        write!(f, ">>{:?}<<", *best_candidate)
    }
}

impl<S: Solution> Hive<S> {
    /// Create a new hive.
    ///
    /// * `workers` - Number of working solution candidates to maintain at a time.
    /// * `observers` - Number of "bees" that will randomly choose a candidate to
    ///                 work on each round.
    /// * `retries` - Number of times a candidate can be worked on without improvement,
    ///               before it will be considered a local maximum and reinitialized.
    pub fn new(mut builder: S::Builder,
               workers: usize,
               observers: usize,
               retries: usize)
               -> Hive<S> {
        if workers == 0 {
            panic!("Hive must have at least one worker.");
        }

        let mut candidates = (0..workers)
                                 .map(|_| Candidate::new(S::make(&mut builder)))
                                 .collect::<Vec<_>>();

        let best = {
            let best_candidate = candidates.iter()
                                           .fold1(|best, next| {
                                               if next.fitness > best.fitness {
                                                   next
                                               } else {
                                                   best
                                               }
                                           })
                                           .unwrap();
            Mutex::new(best_candidate.clone())
        };

        let working = candidates.drain(..)
                                .map(|c| RwLock::new(WorkingCandidate::new(c, retries)))
                                .collect::<Vec<_>>();

        Hive {
            workers: workers,
            observers: observers,
            retries: retries,

            builder: Mutex::new(builder),
            working: working,
            best: best,
            tasks: Mutex::new(None),
            streaming: None,

            threads: num_cpus::get(),
            scale: proportionate(),
        }
    }

    /// Set the number of worker threads to use while running.
    pub fn set_threads(mut self, threads: usize) -> Hive<S> {
        self.threads = threads;
        self
    }

    /// Set the scaling function for observers to use.
    pub fn set_scaling(mut self, scale: Box<ScalingFunction>) -> Hive<S> {
        self.scale = scale;
        self
    }

    fn current_working(&self) -> AbcResult<Vec<Candidate<S>>> {
        let mut current_working = Vec::with_capacity(self.working.len());
        for candidate_mutex in &self.working {
            let read_guard = try!(candidate_mutex.read());
            current_working.push(read_guard.candidate.clone())
        }
        Ok(current_working)
    }

    fn consider_improvement(&self, candidate: &Candidate<S>) -> AbcResult<()> {
        let mut best_guard = try!(self.best.lock());
        if candidate.fitness > best_guard.fitness {
            *best_guard = candidate.clone();
            if let Some(mutex) = self.streaming.as_ref() {
                // We're streaming, so we need to post the improved candidate.
                let sender_guard = try!(mutex.lock());
                // If this errors, the receiver was dropped, so we're done.
                if let Err(_) = sender_guard.send(candidate.clone()) {
                    try!(self.stop());
                }
            }
        }
        Ok(())
    }

    fn work_on(&self, current_working: &[Candidate<S>], n: usize) -> AbcResult<()> {
        let variant = Candidate::new(S::explore(current_working, n));

        let mut write_guard = try!(self.working[n].write());
        if variant.fitness > write_guard.candidate.fitness {
            *write_guard = WorkingCandidate::new(variant, self.retries);
            try!(self.consider_improvement(&write_guard.candidate));
        } else {
            write_guard.deplete();
            // Scouting has been folded into the working process
            if write_guard.expired() {
                let mut builder = try!(self.builder.lock());
                let candidate = Candidate::new(S::make(&mut builder));
                drop(builder);
                *write_guard = WorkingCandidate::new(candidate, self.retries);
                try!(self.consider_improvement(&write_guard.candidate));
            }
        }
        Ok(())
    }

    fn choose(&self, current_working: &[Candidate<S>]) -> usize {
        let fitnesses = (self.scale)(current_working.iter()
                                                    .map(|candidate| candidate.fitness)
                                                    .collect::<Vec<f64>>());

        let running_totals = fitnesses.iter()
                                      .scan(0f64, |total, fitness| {
                                          *total += *fitness;
                                          Some(*total)
                                      })
                                      .collect::<Vec<f64>>();

        let total_fitness = running_totals.last().unwrap();
        let choice_point = thread_rng().next_f64() * total_fitness;

        for (i, total) in running_totals.iter().enumerate() {
            if *total > choice_point {
                return i;
            }
        }
        unreachable!();
    }

    fn execute(&self, task: &Task) -> AbcResult<()> {
        let current_working = try!(self.current_working());
        let index = match *task {
            Task::Worker(n) => n,
            Task::Observer(_) => self.choose(&current_working),
        };
        self.work_on(&current_working, index)
    }

    fn run(&self, tasks: TaskGenerator) -> AbcResult<()> {
        let mut guard = try!(self.tasks.lock());
        *guard = Some(tasks);
        drop(guard);

        let mut handles: Vec<ScopedJoinHandle<AbcResult<()>>> = Vec::with_capacity(self.threads);

        scope(|scope| {
            for _ in 0..self.threads {
                handles.push(scope.spawn(|| {
                    loop {
                        let mut guard = try!(self.tasks.lock());
                        let task = guard.as_mut().and_then(|gen| gen.next());
                        drop(guard);

                        match task {
                            Some(t) => try!(self.execute(&t)),
                            None => return Ok(()),
                        };
                    }
                }));
            }

            // Return `Ok(())` only if all threads join cleanly, and the task
            // cycle is successfully cleared away.
            //
            // We avoid `try!` because we want all of the following logic to
            // execute unconditionally.
            handles.drain(..)
                   .fold(Ok(()), |result, handle| result.and(handle.join()))
                   .and(self.tasks
                            .lock()
                            .map(|mut tasks_guard| *tasks_guard = None)
                            .map_err(AbcError::from))
        })
    }

    /// Runs for a fixed number of rounds, then return the best solution found.
    ///
    /// If one of the worker threads panics while working, this will return
    /// `Err(abc::Error)`. Otherwise, it will return `Ok` with a `Candidate`.
    pub fn run_for_rounds(&self, rounds: usize) -> AbcResult<Candidate<S>> {
        let tasks = TaskGenerator::new(self.workers, self.observers).max_rounds(rounds);
        try!(self.run(tasks));
        self.get().map(|guard| guard.clone())
    }

    /// Runs indefinitely in the background, providing a stream of results.
    ///
    /// This method consumes the hive, which will run until the `Hive` object
    /// is dropped. It returns an `mpsc::Receiver`, which receives a
    /// `Candidate` each time the hive improves on its best solution.
    pub fn stream(mut self) -> Receiver<Candidate<S>> {
        let (sender, receiver) = channel();
        spawn(move || {
            let tasks = TaskGenerator::new(self.workers, self.observers);
            self.streaming = Some(Mutex::new(sender));
            self.run(tasks)
        });
        receiver
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
    /// #     type Builder = ();
    /// #     fn make(_: &mut ()) -> X { X }
    /// #     fn evaluate_fitness(&self) -> f64 { 0_f64 }
    /// #     fn explore(field: &[Candidate<X>], n: usize) -> X { X }
    /// # }
    /// # fn do_stuff_with(x: Candidate<X>) {}
    /// # fn main() {
    /// let hive: Hive<X> = Hive::new((), 5, 5, 5);
    /// let current_best = {
    ///     let guard = hive.get().unwrap();
    ///     guard.clone()
    /// };
    ///
    /// do_stuff_with(current_best);
    /// # }
    /// ```
    pub fn get(&self) -> AbcResult<MutexGuard<Candidate<S>>> {
        self.best.lock().map_err(AbcError::from)
    }

    /// Stops a running hive.
    ///
    /// If a worker thread has panicked, this returns `Err(abc::Error)`.
    pub fn stop(&self) -> AbcResult<()> {
        let mut tasks_guard = try!(self.tasks.lock());
        Ok(tasks_guard.as_mut().map_or((), |t| t.stop()))
    }

    /// Returns the current round of a running hive.
    ///
    /// If a worker thread has panicked and poisoned the task generator lock,
    /// `get_round` will return `Err(abc::Error)`.
    ///
    /// If the hive has not been run, `get_round` will return `Ok(None)`.
    pub fn get_round(&self) -> AbcResult<Option<usize>> {
        let tasks_guard = try!(self.tasks.lock());
        Ok(tasks_guard.as_ref().map(|tasks| tasks.round))
    }
}

impl<S: Solution> Drop for Hive<S> {
    fn drop(&mut self) {
        self.stop().unwrap()
    }
}
