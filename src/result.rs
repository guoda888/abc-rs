use std::result;
use std::sync::{PoisonError, MutexGuard, RwLockReadGuard, RwLockWriteGuard};
use std::fmt;
use std::error;

#[derive(Debug)]
/// Unifies the errors thrown by a hive's operation.
///
/// The only errors expected within the hive code are associated with getting
/// read and/or write locks on aspects of the hive's data. These errors occur
/// if a thread panics while holding the lock -- a situation that we do not
/// particularly expect.
///
/// Nevertheless, this `Error` represents a panic, most likely in one of the
/// hive's worker threads.
pub struct Error;

impl error::Error for Error {
    fn description(&self) -> &str {
        "One of the hive's workers panicked."
    }

    fn cause(&self) -> Option<&error::Error> {
        None
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "One of the hive's workers panicked.")
    }
}

// Each type of guard (mutex, read, and write) has its own parameterized form
// of the PoisonError. Since they all amount to the same thing for our purposes,
// we abstract over them with T.
impl<T> From<PoisonError<T>> for Error {
    fn from(err: PoisonError<T>) -> Error {
        Error
    }
}

/// Encodes the possibility of a thread panicking and corruping a mutex.
pub type Result<T> = result::Result<T, Error>;