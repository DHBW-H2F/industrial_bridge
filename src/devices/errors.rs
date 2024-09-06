use custom_error::custom_error;
use serde_json;
use std::{error::Error, net::AddrParseError};

custom_error! {pub DeviceInitError
    CouldNotOpenDefinition{ err: Box<dyn Error>} = "Could not find definition file ({err})",
    ParsingFailed{ err: Box<dyn Error> } = "Could not parse file ({err})",
    BadRemoteUri{ err: Box<dyn Error> } = "Could not get a correct URL from passed remote address ({err})",
}

impl From<std::io::Error> for DeviceInitError {
    fn from(value: std::io::Error) -> Self {
        DeviceInitError::CouldNotOpenDefinition {
            err: Box::new(value),
        }
    }
}

impl From<serde_json::error::Error> for DeviceInitError {
    fn from(value: serde_json::error::Error) -> Self {
        DeviceInitError::ParsingFailed {
            err: Box::new(value),
        }
    }
}

impl From<AddrParseError> for DeviceInitError {
    fn from(value: AddrParseError) -> Self {
        DeviceInitError::BadRemoteUri {
            err: Box::new(value),
        }
    }
}
