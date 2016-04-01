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
use std::collections::BTreeSet;

use task::{TaskGenerator, Task};
use candidate::{WorkingCandidate, Candidate};
use context::Context;
use scaling::{ScalingFunction, proportionate};
use result::{Result as AbcResult, Error as AbcError};

/// Manages the parameters of the ABC algorithm.
pub struct HiveBuilder<Ctx: Context> {
    workers: usize,
    observers: usize,
    retries: usize,
    context: Ctx,
    threads: usize,
    scale: Box<ScalingFunction>,
}

impl<Ctx: Context> HiveBuilder<Ctx> {
    /// Creates a new hive.
    ///
    /// * `context` - Factory-like state that can be used while generating solutions.
    /// * `workers` - Number of working solution candidates to maintain at a time.
    pub fn new(context: Ctx, workers: usize) -> HiveBuilder<Ctx> {
        if workers == 0 {
            panic!("HiveBuilder must have at least one worker.");
        }

        HiveBuilder {
            workers: workers,
            observers: workers,
            retries: workers,

            context: context,
            threads: num_cpus::get(),
            scale: proportionate(),
        }
    }

    /// Sets the number of "bees" that will pick a candidate to work on at random.
    ///
    /// This defaults to the number of workers.
    pub fn set_observers(mut self, observers: usize) -> HiveBuilder<Ctx> {
        self.observers = observers;
        self
    }

    /// Sets the number of times a candidate can go unimproved before being reinitialized.
    ///
    /// This defaults to the number of workers.
    pub fn set_retries(mut self, retries: usize) -> HiveBuilder<Ctx> {
        self.retries = retries;
        self
    }

    /// Sets the number of worker threads to use while running.
    pub fn set_threads(mut self, threads: usize) -> HiveBuilder<Ctx> {
        self.threads = threads;
        self
    }

    /// Sets the scaling function for observers to use.
    pub fn set_scaling(mut self, scale: Box<ScalingFunction>) -> HiveBuilder<Ctx> {
        self.scale = scale;
        self
    }

    /// Activates the `HiveBuilder` to create a runnable object.
    pub fn build(self) -> AbcResult<Hive<Ctx>> {
        Hive::new(self)
    }

    fn new_candidate(&self) -> Candidate<Ctx::Solution> {
        let solution = self.context.make();
        let fitness = self.context.evaluate_fitness(&solution);
        Candidate::new(solution, fitness)
    }
}

/// Runs the ABC algorithm, maintaining any necessary state.
pub struct Hive<Ctx: Context> {
    hive: HiveBuilder<Ctx>,

    working: Vec<RwLock<WorkingCandidate<Ctx::Solution>>>,
    best: Mutex<Candidate<Ctx::Solution>>,
    scouting: RwLock<BTreeSet<usize>>,

    tasks: Mutex<Option<TaskGenerator>>,
    sender: Option<Mutex<Sender<Candidate<Ctx::Solution>>>>,
}

impl<Ctx: Context> Hive<Ctx> {
    fn new(hive: HiveBuilder<Ctx>) -> AbcResult<Hive<Ctx>> {
        // Start by populating the field with an initial set of solution candidates.

        // Feed the worker threads a total of N items, each signifying that
        // we need another candidate.
        let tokens: Mutex<Range<usize>> = Mutex::new(0..hive.workers);

        let candidates = Mutex::new(Vec::with_capacity(hive.workers));
        let mut handles = Vec::<ScopedJoinHandle<AbcResult<()>>>::with_capacity(hive.threads);

        try!(crossbeam::scope(|scope| {
            for _ in 0..hive.threads {
                handles.push(scope.spawn(|| {
                    while let Some(_) = {
                        let mut guard = tokens.lock().unwrap();
                        guard.next()
                    } {
                        let candidate = hive.new_candidate();
                        try!(candidates.lock()).push(candidate);
                    }
                    Ok(())
                }));
            }

            // Gather and return `Ok` iff all of the workers finished
            // successfully, otherwise abort the construction.
            handles.drain(..)
                   .fold(Ok(()), |result, handle| result.and(handle.join()))
        }));

        // We don't need the mutex anymore, since we're no longer populating
        // the candidate set from multiple threads.
        let mut candidates = try!(candidates.into_inner());

        // Find the current best candidate, since we want to cache the best
        // at any given moment.
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

        // Wrap the candidates in a structure that will let the eventual
        // thread swarm work on them.
        let working = candidates.drain(..)
                                .map(|c| RwLock::new(WorkingCandidate::new(c, hive.retries)))
                                .collect::<Vec<RwLock<WorkingCandidate<Ctx::Solution>>>>();

        Ok(Hive {
            hive: hive,
            working: working,
            best: best,
            scouting: RwLock::new(BTreeSet::new()),
            tasks: Mutex::new(None),
            sender: None,
        })
    }

    /// Clone a snapshot of the current set of working candidates.
    ///
    /// The goal of this function is to hold a guard for each solution for as
    /// little time as possible, so we can get out of the way of the other
    /// threads. To this end, we clone the solutions, so that the thread can do
    /// its work on a snapshot.
    fn current_working(&self) -> AbcResult<Vec<Candidate<Ctx::Solution>>> {
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
    pub fn get(&self) -> AbcResult<MutexGuard<Candidate<Ctx::Solution>>> {
        self.best.lock().map_err(AbcError::from)
    }

    /// Perform greedy selection between a new candidate and the current best.
    fn consider_improvement(&self, candidate: &Candidate<Ctx::Solution>) -> AbcResult<()> {
        let mut best_guard = try!(self.best.lock());
        if candidate.fitness > best_guard.fitness {
            *best_guard = candidate.clone();
            if let Some(mutex) = self.sender.as_ref() {
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

    fn work_on(&self, current_working: &[Candidate<Ctx::Solution>], n: usize) -> AbcResult<()> {
        let variant_solution = self.hive.context.explore(current_working, n);
        let variant_fitness = self.hive.context.evaluate_fitness(&variant_solution);
        let variant = Candidate::new(variant_solution, variant_fitness);
        let mut write_guard = try!(self.working[n].write());
        if variant.fitness > write_guard.candidate.fitness {
            *write_guard = WorkingCandidate::new(variant, self.hive.retries);
            try!(self.consider_improvement(&write_guard.candidate));
        } else {
            write_guard.deplete();
            // Scouting has been folded into the working process
            if write_guard.expired() {
                let mut scouting_guard = try!(self.scouting.write());
                scouting_guard.insert(n);
                drop(scouting_guard);
                drop(write_guard);

                let candidate = self.hive.new_candidate();
                let mut write_guard = try!(self.working[n].write());
                *write_guard = WorkingCandidate::new(candidate, self.hive.retries);
                try!(self.consider_improvement(&write_guard.candidate));
                drop(write_guard);

                let mut scouting_guard = try!(self.scouting.write());
                scouting_guard.remove(&n);
            }
        }
        Ok(())
    }

    fn choose(&self, current_working: &[Candidate<Ctx::Solution>]) -> AbcResult<usize> {
        let fitnesses = (self.hive.scale)(current_working.iter()
                                                         .map(|candidate| candidate.fitness)
                                                         .collect::<Vec<f64>>());

        // Avoid observing candidates that are being scouted.
        let scouting_guard = try!(self.scouting.read());
        let running_totals = fitnesses.iter()
                                      .enumerate()
                                      .filter(|&(ref i, _)| !scouting_guard.contains(i))
                                      .scan(0f64, |total, (i, fitness)| {
                                          *total += *fitness;
                                          Some((i, *total))
                                      })
                                      .collect::<Vec<(usize, f64)>>();
        drop(scouting_guard);

        // Multiplying the choice point is equivalent to, and more efficient than, normalizing
        // all of the scaled fitnesses and having a choice point in [0,1)
        match running_totals.last() {
            Some(&(_, total_fitness)) => {
                let choice_point = thread_rng().next_f64() * total_fitness;
                for &(i, total) in &running_totals {
                    if total > choice_point {
                        return Ok(i);
                    }
                }
                unreachable!();
            }

            // If we are currently scouting all of the solutions, pick one at random.
            None => Ok(thread_rng().gen_range::<usize>(0, fitnesses.len())),
        }
    }

    fn execute(&self, task: &Task) -> AbcResult<()> {
        let current_working = try!(self.current_working());
        let index = match *task {
            Task::Worker(n) => {
                // If the worker's candidate is in the middle of being replaced, just skip it.
                let scouting_guard = try!(self.scouting.read());
                if scouting_guard.contains(&n) {
                    return Ok(());
                }
                n
            }
            Task::Observer(_) => try!(self.choose(&current_working)),
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
    pub fn run_for_rounds(&self, rounds: usize) -> AbcResult<Candidate<Ctx::Solution>> {
        let tasks = TaskGenerator::new(self.hive.workers, self.hive.observers).max_rounds(rounds);
        try!(self.run(tasks));
        self.get().map(|guard| guard.clone())
    }

    /// Run indefinitely.
    ///
    /// If one of the worker threads panics while working, this will return
    /// `Err(abc::Error)`. Otherwise, it will return `Ok(())`.
    pub fn run_forever(&self) -> AbcResult<()> {
        let tasks = TaskGenerator::new(self.hive.workers, self.hive.observers);
        self.run(tasks)
    }

    /// Stops a running hive.
    ///
    /// If a worker thread has panicked, this returns `Err(abc::Error)`.
    pub fn stop(&self) -> AbcResult<()> {
        let mut tasks_guard = try!(self.tasks.lock());
        Ok(tasks_guard.as_mut().map_or((), |t| t.stop()))
    }

    /// Each new best candidate will be sent to `sender`.
    ///
    /// This is kept in a separate function so that the hive can be borrowed
    /// while running.
    pub fn set_sender(&mut self, sender: Sender<Candidate<Ctx::Solution>>) {
        if let Ok(best_guard) = self.best.lock() {
            sender.send(best_guard.clone()).unwrap_or(());
        }
        self.sender = Some(Mutex::new(sender));
    }

    /// Returns the current round of a running hive.
    ///
    /// If a worker thread has panicked and poisoned the task generator lock,
    /// `get_round` will return `Err(abc::Error)`. If the hive has not been
    /// run, `get_round` will return `Ok(None)`.
    ///
    /// If the hive is running, this will return `Ok(Some(n))`. `n` will start
    /// at 0, and increment each time every task in the round has been claimed
    /// (though not necessarily completed) by a worker thread.
    pub fn get_round(&self) -> AbcResult<Option<usize>> {
        let tasks_guard = try!(self.tasks.lock());
        Ok(tasks_guard.as_ref().map(|tasks| tasks.round))
    }

    /// Get a reference to the hive's context.
    pub fn context(&self) -> &Ctx {
        &self.hive.context
    }
}

impl<Ctx: Context + 'static> Hive<Ctx> {
    /// Runs indefinitely in the background, providing a stream of results.
    ///
    /// This method consumes the hive, which will run until the `HiveBuilder`
    /// object is dropped. It returns an `mpsc::Receiver`, which receives a
    /// `Candidate` each time the hive improves on its best solution.
    pub fn stream(mut self) -> Receiver<Candidate<Ctx::Solution>> {
        let (sender, receiver) = channel();
        spawn(move || {
            self.set_sender(sender);
            let tasks = TaskGenerator::new(self.hive.workers, self.hive.observers);
            self.run(tasks)
        });
        receiver
    }
}

impl<Ctx: Context> Debug for Hive<Ctx>
    where Ctx::Solution: Debug
{
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        for mutex in (&self.working).iter() {
            let working = mutex.read().unwrap();
            try!(write!(f, "..{:?}..\n", working.candidate));
        }
        let best_candidate = self.get().unwrap();
        write!(f, ">>{:?}<<", *best_candidate)
    }
}

impl<Ctx: Context> Drop for Hive<Ctx> {
    fn drop(&mut self) {
        self.stop().unwrap_or(())
    }
}
