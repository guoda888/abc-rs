use solution::Solution;
use std::mpsc;

trait BestSolutionAdapter<S: Solution> {
    fn new_best(&self, item: S);
}

impl<S: Solution> BestSolutionAdapter<S> for mpsc::Sender<S> {
    fn new_best(&self, item: S) {
        self.send(item)
    }
}