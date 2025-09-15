use std::collections::HashMap;

use custom_error::custom_error;
use prometheus_push::error::PushMetricsError;

use crate::types_conversion::RegisterValue;

use async_trait::async_trait;

custom_error! {
    /// List of error related to the push of the data to the remote
    pub RemoteError
    DisconnectedRemoteError = "The remote is not connected",
    PushFailedError{ res: String } = "There was an error during push : {res}",
    AuthError = "Authentification error",
    ServerError = "Server error",
    QueryError = "Query error",
}

impl From<PushMetricsError> for RemoteError {
    fn from(value: PushMetricsError) -> Self {
        match value {
            PushMetricsError::Url(_) => RemoteError::ServerError,
            PushMetricsError::AlreadyContainsLabel(_) => RemoteError::QueryError,
            PushMetricsError::SlashInName(_) => RemoteError::QueryError,
            PushMetricsError::Prometheus(_) => RemoteError::ServerError,
            PushMetricsError::Response(_) => RemoteError::QueryError,
            PushMetricsError::Reqwest(_) => RemoteError::QueryError,
        }
    }
}

impl From<influxdb::Error> for RemoteError {
    fn from(value: influxdb::Error) -> Self {
        match value {
            influxdb::Error::InvalidQueryError { error: _error } => RemoteError::QueryError,
            influxdb::Error::UrlConstructionError { error: _error } => RemoteError::QueryError,
            influxdb::Error::ProtocolError { error: _error } => RemoteError::QueryError,
            influxdb::Error::DeserializationError { error: _error } => RemoteError::QueryError,
            influxdb::Error::DatabaseError { error: _error } => RemoteError::ServerError,
            influxdb::Error::AuthenticationError => RemoteError::AuthError,
            influxdb::Error::AuthorizationError => RemoteError::AuthError,
            influxdb::Error::ConnectionError { error: _error } => {
                RemoteError::DisconnectedRemoteError
            }
        }
    }
}

#[async_trait]
/// Interface to describe the remote where we send all the collected data
pub trait Remote {
    async fn send_measurement(
        &self,
        name: &str,
        values: &HashMap<String, RegisterValue>,
    ) -> Result<(), RemoteError>;
}
