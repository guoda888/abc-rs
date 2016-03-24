extern crate num_cpus;
extern crate itertools;
extern crate rand;
extern crate crossbeam;

use self::rand::{thread_rng, Rng};
use self::itertools::Itertools;
use self::crossbeam::{scope, ScopedJoinHandle};

use std::ops::Range;
use std::fmt::{Debug, Formatter, Result as FmtResult};
use std::sync::{Mutex, RwLock, MutexGuard};
use std::sync::mpsc::{Sender, Receiver, channel};
use std::thread::spawn;

use task::{TaskGenerator, Task};
use candidate::{WorkingCandidate, Candidate};
use solution::Solution;
use scaling::{ScalingFunction, proportionate};
use result::{Result as AbcResult, Error as AbcError};

/// Manages the parameters of the ABC algorithm.
pub struct HiveBuilder<S: Solution> {
    workers: usize,
    observers: usize,
    retries: usize,
    builder: Mutex<S::Builder>,
    threads: usize,
    scale: Box<ScalingFunction>,
}

impl<S: Solution> HiveBuilder<S> {
    /// Creates a new hive.
    ///
    /// * `builder` - Factory-like state that can be used while generating solutions.
    /// * `workers` - Number of working solution candidates to maintain at a time.
    pub fn new(builder: S::Builder, workers: usize) -> HiveBuilder<S> {
        if workers == 0 {
            panic!("HiveBuilder must have at least one worker.");
        }

        HiveBuilder {
            workers: workers,
            observers: workers,
            retries: workers,

            builder: Mutex::new(builder),
            threads: num_cpus::get(),
            scale: proportionate(),
        }
    }

    /// Sets the number of "bees" that will pick a candidate to work on at random.
    ///
    /// This defaults to the number of workers.
    pub fn set_observers(mut self, observers: usize) -> HiveBuilder<S> {
        self.observers = observers;
        self
    }

    /// Sets the number of times a candidate can go unimproved before being reinitialized.
    ///
    /// This defaults to the number of workers.
    pub fn set_retries(mut self, retries: usize) -> HiveBuilder<S> {
        self.retries = retries;
        self
    }

    /// Sets the number of worker threads to use while running.
    pub fn set_threads(mut self, threads: usize) -> HiveBuilder<S> {
        self.threads = threads;
        self
    }

    /// Sets the scaling function for observers to use.
    pub fn set_scaling(mut self, scale: Box<ScalingFunction>) -> HiveBuilder<S> {
        self.scale = scale;
        self
    }

    /// Activates the `HiveBuilder` to create a runnable object.
    pub fn build(self) -> AbcResult<Hive<S>> {
        Hive::new(self)
    }
}

/// Runs the ABC algorithm, maintaining any necessary state.
pub struct Hive<S: Solution> {
    hive: HiveBuilder<S>,

    working: Vec<RwLock<WorkingCandidate<S>>>,
    best: Mutex<Candidate<S>>,

    tasks: Mutex<Option<TaskGenerator>>,
    streaming: Option<Mutex<Sender<Candidate<S>>>>,
}

impl<S: Solution> Hive<S> {
    fn new(hive: HiveBuilder<S>) -> AbcResult<Hive<S>> {
        let tokens: Mutex<Range<usize>> = Mutex::new(0..hive.workers);
        let candidates = Mutex::new(Vec::with_capacity(hive.workers));
        let mut handles = Vec::<ScopedJoinHandle<AbcResult<()>>>::with_capacity(hive.threads);

        try!(crossbeam::scope(|scope| {
            for _ in 0..hive.threads {
                handles.push(scope.spawn(|| {
                    while let Some(_) = tokens.lock().unwrap().next() {
                        let mut builder = try!(hive.builder.lock());
                        let solution = S::make(&mut builder);
                        drop(builder);
                        let candidate = Candidate::new(solution);
                        try!(candidates.lock()).push(candidate);
                    }
                    Ok(())
                }));
            }

            handles.drain(..)
                   .fold(Ok(()), |result, handle| result.and(handle.join()))
        }));

        let mut candidates = try!(candidates.into_inner());

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
                                .map(|c| RwLock::new(WorkingCandidate::new(c, hive.retries)))
                                .collect::<Vec<_>>();

        Ok(Hive {
            working: working,
            best: best,
            hive: hive,
            tasks: Mutex::new(None),
            streaming: None,
        })
    }

    fn current_working(&self) -> AbcResult<Vec<Candidate<S>>> {
        let mut current_working = Vec::with_capacity(self.working.len());
        for candidate_mutex in &self.working {
            let read_guard = try!(candidate_mutex.read());
            current_working.push(read_guard.candidate.clone())
        }
        Ok(current_working)
    }

    /// Returns a guard for the current best solution found by the hive.
    ///
    /// If the hive is running, you should drop the guard returned by this
    /// function as soon as convenient, since the logic of the hive can block
    /// on the availability of the associated mutex. If you plan on performing
    /// expensive computations, you should `drop` the guard as soon as
    /// possible, or acquire and clone it within a small block.
    pub fn get(&self) -> AbcResult<MutexGuard<Candidate<S>>> {
        self.best.lock().map_err(AbcError::from)
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
            *write_guard = WorkingCandidate::new(variant, self.hive.retries);
            try!(self.consider_improvement(&write_guard.candidate));
        } else {
            write_guard.deplete();
            // Scouting has been folded into the working process
            if write_guard.expired() {
                let mut builder = try!(self.hive.builder.lock());
                let solution = S::make(&mut builder);
                drop(builder);
                let candidate = Candidate::new(solution);
                *write_guard = WorkingCandidate::new(candidate, self.hive.retries);
                try!(self.consider_improvement(&write_guard.candidate));
            }
        }
        Ok(())
    }

    fn choose(&self, current_working: &[Candidate<S>]) -> usize {
        let fitnesses = (self.hive.scale)(current_working.iter()
                                                         .map(|candidate| candidate.fitness)
                                                         .collect::<Vec<f64>>());

        let running_totals = fitnesses.iter()
                                      .scan(0f64, |total, fitness| {
                                          *total += *fitness;
                                          Some(*total)
                                      })
                                      .collect::<Vec<f64>>();

        // Multiplying the choice point is equivalent to, and more efficient than, normalizing
        // all of the scaled fitnesses and having a choice point in [0,1)
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

        let mut handles: Vec<ScopedJoinHandle<AbcResult<()>>> = Vec::new();

        scope(|scope| {
            for _ in 0..self.hive.threads {
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

            // Returns `Ok(())` only if all threads join cleanly, and the task
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
        let tasks = TaskGenerator::new(self.hive.workers, self.hive.observers).max_rounds(rounds);
        try!(self.run(tasks));
        self.get().map(|guard| guard.clone())
    }

    /// Runs indefinitely in the background, providing a stream of results.
    ///
    /// This method consumes the hive, which will run until the `HiveBuilder` object
    /// is dropped. It returns an `mpsc::Receiver`, which receives a
    /// `Candidate` each time the hive improves on its best solution.
    pub fn stream(mut self) -> Receiver<Candidate<S>> {
        let (sender, receiver) = channel();
        spawn(move || {
            let tasks = TaskGenerator::new(self.hive.workers, self.hive.observers);
            self.streaming = Some(Mutex::new(sender));
            self.run(tasks)
        });
        receiver
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

impl<S: Solution> Drop for Hive<S> {
    fn drop(&mut self) {
        self.stop().unwrap_or(())
    }
}
