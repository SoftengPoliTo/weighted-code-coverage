use std::sync::PoisonError;

use crossbeam::channel::SendError;
use thiserror::Error;

/// Customized error messages using thiserror library
#[derive(Error, Debug)]
pub enum Error {
    #[error("Error while reading Files from project folder")]
    WrongFile(#[from] std::io::Error),
    #[error("Error while reading json")]
    WrongJSONFile(#[from] serde_json::Error),
    #[error("Error while converting JSON value to a type")]
    Conversion,
    #[error("Error while getting value from hashmap")]
    HashMap,
    #[error("Failing reading JSON from string")]
    ReadingJSON,
    #[error("Error while computing Metrics")]
    Metrics,
    #[error("Error while guessing language")]
    Language,
    #[error("Error while writing on csv")]
    Writing(#[from] csv::Error),
    #[error("Error during concurrency")]
    Concurrent,
    #[error("Json Type is not supported! Only coveralls and covdir are supported.")]
    Type,
    #[error("Error while converting path to string")]
    PathConversion,
    #[error("Error while locking mutex")]
    Mutex,
    #[error("Thresholds must be only 4 in this order -t WCC_PLAIN, WCC_QUANTIZED, CRAP, SKUNK")]
    Thresholds,
    #[error("Error while sending job via sender")]
    Sender,
    #[error("Error while creating HTML file")]
    Html(#[from] tera::Error),
}

pub(crate) type Result<T> = ::std::result::Result<T, Error>;

impl<T> From<PoisonError<T>> for Error {
    fn from(_item: PoisonError<T>) -> Self {
        Error::Mutex
    }
}

impl From<Box<dyn std::any::Any + Send>> for Error {
    fn from(_item: Box<dyn std::any::Any + Send>) -> Self {
        Error::Concurrent
    }
}

impl<T> From<SendError<T>> for Error {
    fn from(_item: SendError<T>) -> Self {
        Error::Sender
    }
}
