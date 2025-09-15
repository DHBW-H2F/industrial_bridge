use std::collections::HashMap;

use prometheus::Gauge;
use prometheus_push::prometheus_crate::PrometheusMetricsPusher;
use serde::Deserialize;
use url::Url;

use crate::remotes::remote::RemoteError;
use crate::remotes::Remote;

use async_trait::async_trait;

use super::errors::RemoteInitError;

#[async_trait]
impl Remote for PrometheusMetricsPusher {
    /// Sends a measurement to the remote prometheus instance.
    ///
    /// Builds an prometheus query using the current local timestamp and
    /// appends all provided register values as fields of the measurement.
    ///
    /// Parameters
    /// - `name`: the name of the measurement (prometheus series name).
    /// - `values`: a map of field names to `RegisterValue`s that will be
    ///   converted and stored as fields in the measurement.
    ///
    /// Returns
    /// - `Ok(())` if the measurement was successfully pushed.
    /// - `Err(RemoteError)` if the push failed or the server returned an error.
    ///
    /// Errors
    /// - `RemoteError::PushFailedError` if prometheus responded with a non-empty error result.
    /// - Propagates other errors returned from the underlying query execution.
    async fn send_measurement(
        &self,
        name: &str,
        values: &std::collections::HashMap<String, crate::types_conversion::RegisterValue>,
    ) -> Result<(), RemoteError> {
        let registry = prometheus::Registry::new();
        for (field, value) in values {
            let gauge =
                Gauge::new(field.replace(&['-', '/', '[', ']', '%'][..], "_"), field).unwrap();
            gauge.set(value.clone().into());
            registry.register(Box::new(gauge)).unwrap();
        }

        self.push_all(name, &HashMap::new(), registry.gather())
            .await?;

        Ok(())
    }
}

#[derive(Deserialize, Debug)]
pub struct PrometheusRemote {
    pub remote: String,
}

impl TryFrom<PrometheusRemote> for PrometheusMetricsPusher {
    type Error = RemoteInitError;

    fn try_from(value: PrometheusRemote) -> Result<Self, Self::Error> {
        let client = reqwest::Client::new();
        let pusher = PrometheusMetricsPusher::from(client, &Url::parse(&value.remote)?)?;
        Ok(pusher)
    }
}
