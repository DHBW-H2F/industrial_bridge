use std::collections::HashMap;

use prometheus::Gauge;
use prometheus_push::prometheus_crate::PrometheusMetricsPusher;

use crate::remotes::remote::RemoteError;
use crate::remotes::Remote;

use async_trait::async_trait;

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
