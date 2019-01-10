//! Error types and utilities.

use region;
use std::fmt;

/// The result of an allocation.
pub type Result<T> = ::std::result::Result<T, Error>;

/// A collection of possible errors.
#[derive(Debug)]
pub enum Error {
  /// The system is out of executable memory.
  OutOfMemory,
  /// A memory query failed.
  RegionFailure(region::Error),
}

impl fmt::Display for Error {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match self {
      Error::OutOfMemory => write!(f, "Cannot allocate memory"),
      Error::RegionFailure(ref error) => write!(f, "{}", error),
    }
  }
}

impl From<region::Error> for Error {
  fn from(error: region::Error) -> Self {
    Error::RegionFailure(error)
  }
}
