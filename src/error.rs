use std::sync::PoisonError;

use csv;
use serde_json;
use tera;
use thiserror::Error;

/// Customized error messages using thiserror library
#[derive(Error, Debug)]
pub enum Error {
    #[error("Error while reading Files from project folder")]
    WrongFile(#[from] std::io::Error),
    #[error("Error while reading json")]
    WrongJSONFile(#[from] serde_json::Error),
    #[error("Error while converting JSON value to a type")]
    ConversionError(),
    #[error("Error while getting value from hashmap")]
    HashMapError(),
    #[error("Failing reading JSON from string")]
    ReadingJSONError(),
    #[error("Error while computing Metrics")]
    MetricsError(),
    #[error("Error while guessing language")]
    LanguageError(),
    #[error("Error while writing on csv")]
    WritingError(#[from] csv::Error),
    #[error("Error during concurrency")]
    ConcurrentError(),
    #[error("Json Type is not supported! Only coveralls and covdir are supported.")]
    TypeError(),
    #[error("Error while converting path to string")]
    PathConversionError(),
    #[error("Error while locking mutex")]
    MutexError(),
    #[error("Thresholds must be only 4 in this order -t WCC_PLAIN, WCC_QUANTIZED, CRAP, SKUNK")]
    ThresholdsError(),
    #[error("Error while sending job via sender")]
    SenderError(),
    #[error("Error while creating HTML file")]
    HTMLError(#[from] tera::Error),
}

pub type Result<T> = ::std::result::Result<T, Error>;

impl<T> From<PoisonError<T>> for Error {
    fn from(_item: PoisonError<T>) -> Self {
        Error::MutexError()
    }
}

impl From<Box<dyn std::any::Any + Send>> for Error {
    fn from(_item: Box<dyn std::any::Any + Send>) -> Self {
        Error::ConcurrentError()
    }
}
