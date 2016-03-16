#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Task {
    Worker(usize),
    Observer(usize), // The index is used for cycling, disregarded at execution.
}

pub struct TaskGenerator {
    workers: usize,
    observers: usize,
    next: Task,
    max_rounds: Option<usize>,
    stopped: bool,
    pub round: usize,
}

impl TaskGenerator {
    pub fn new(workers: usize, observers: usize) -> TaskGenerator {
        if workers == 0 {
            panic!("Expect at least one worker.");
        }
        TaskGenerator {
            workers: workers,
            observers: observers,
            round: 0,
            max_rounds: None,
            next: Task::Worker(0),
            stopped: false,
        }
    }

    pub fn max_rounds(mut self, max_rounds: usize) -> TaskGenerator {
        self.max_rounds = Some(max_rounds);
        self
    }

    pub fn stop(&mut self) {
        self.stopped = true;
    }
}

impl Iterator for TaskGenerator {
    type Item = Task;
    fn next(&mut self) -> Option<Self::Item> {
        if !self.stopped {
            let current = self.next.clone();
            self.next = match self.next {
                Task::Worker(n) if n == self.workers - 1 => {
                    if self.observers > 0 {
                        Task::Observer(0)
                    } else {
                        Task::Worker(0)
                    }
                }
                Task::Worker(n) => Task::Worker(n + 1),
                Task::Observer(n) if n == self.observers - 1 => {
                    self.round += 1;
                    if let Some(n) = self.max_rounds {
                        if self.round >= n {
                            self.stopped = true;
                        }
                    }
                    Task::Worker(0)
                }
                Task::Observer(n) => Task::Observer(n + 1),
            };
            Some(current)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn basic_cycle() {
        use super::*;
        let tg = TaskGenerator::new(3, 2).max_rounds(2);
        let gathered: Vec<_> = tg.collect();
        let expected = [
            Task::Worker(0),
            Task::Worker(1),
            Task::Worker(2),
            Task::Observer(0),
            Task::Observer(1),
            Task::Worker(0),
            Task::Worker(1),
            Task::Worker(2),
            Task::Observer(0),
            Task::Observer(1),
        ];
        assert_eq!(gathered.len(), expected.len());
        assert!(gathered.iter().zip(expected.iter()).all(|(x, y)| *x == *y));
    }
}