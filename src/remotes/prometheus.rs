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
