use crate::remotes::remote::RemoteError;
use crate::remotes::Remote;

use async_trait::async_trait;
use influxdb::{Client, InfluxDbWriteable, Type};

#[async_trait]
impl Remote for Client {
    async fn send_measurement(
        &self,
        name: &str,
        values: &std::collections::HashMap<String, crate::types_conversion::RegisterValue>,
    ) -> Result<(), RemoteError> {
        let mut query = influxdb::Timestamp::from(chrono::offset::Local::now()).into_query(name);
        for (field, value) in values {
            query = query.add_field(field, Into::<Type>::into(value.clone()));
        }

        match self.query(query).await {
            Ok(res) => {
                if !res.is_empty() {
                    return Err(RemoteError::PushFailedError { res });
                }
            }
            Err(err) => return Err(err.into()),
        };
        Ok(())
    }
}
