use std::{convert::Infallible, error::Error};

use custom_error::custom_error;
use prometheus_push::error::PushMetricsError;

custom_error! {
    /// List of error related to the config of the remote
    pub RemoteInitError
    ParsingFailed{ err: Box<dyn Error>} = "There was an error parsing",
    InitialisationError{ err: Box<dyn Error> } = "The was an error on initilaisation",
    NotReachable{} = "This should not happen",
}

impl From<url::ParseError> for RemoteInitError {
    fn from(value: url::ParseError) -> Self {
        RemoteInitError::ParsingFailed {
            err: Box::new(value),
        }
    }
}

impl From<PushMetricsError> for RemoteInitError {
    fn from(value: PushMetricsError) -> Self {
        RemoteInitError::InitialisationError {
            err: Box::new(value),
        }
    }
}

impl From<Infallible> for RemoteInitError {
    fn from(_value: Infallible) -> Self {
        RemoteInitError::NotReachable {}
    }
}
