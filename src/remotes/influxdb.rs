use std::convert::Infallible;

use crate::remotes::remote::RemoteError;
use crate::remotes::Remote;

use async_trait::async_trait;
use influxdb::{Client, InfluxDbWriteable, Type};
use serde::Deserialize;

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

#[derive(Deserialize, Debug)]
pub struct InfluxDBRemote {
    pub remote: String,
    pub bucket: String,
    pub token: String,
}

impl TryFrom<InfluxDBRemote> for Client {
    type Error = Infallible;

    fn try_from(value: InfluxDBRemote) -> Result<Self, Self::Error> {
        let client = Client::new(value.remote, value.bucket).with_token(value.token);
        Ok(client)
    }
}
