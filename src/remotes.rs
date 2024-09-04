use std::{collections::HashMap, sync::Arc};

use log::{error, info, warn};
use tokio::{
    select,
    sync::{watch, Mutex},
    task::JoinSet,
};

use crate::types_conversion::RegisterValue;

mod influxdb;
mod prometheus;
pub mod remote;
use remote::{Remote, RemoteError};

async fn join_remotes_tasks(set: &mut JoinSet<Result<(), RemoteError>>) {
    while let Some(result) = set.join_next().await {
        match result {
            Ok(val) => match val {
                Ok(_) => (),
                Err(err) => error!("Could not send data to remote : {err}"),
            },
            Err(err) => {
                error!("There was an error joining the tasks responsible for pushing data ({err})")
            }
        }
    }
}

pub async fn send_data_to_remotes(
    remotes: Arc<Mutex<HashMap<String, Arc<Mutex<impl Remote + Send + 'static + ?Sized>>>>>,
    mut data: watch::Receiver<HashMap<String, HashMap<String, RegisterValue>>>,
) {
    loop {
        info!("New data available : starting push");

        let mut set = JoinSet::new();

        for (name, remote) in remotes.lock().await.iter() {
            let name = name.clone();
            let remote = remote.clone();
            let data_c = data.borrow().clone();
            set.spawn(async move {
                let name: String = name.to_string();
                send_data_to_remote(&name, remote, &data_c).await
            });
        }

        select! {
            _ = join_remotes_tasks(&mut set) => {
                match data.changed().await {
                    Ok(_) => {},
                    Err(err) => error!("There was an error waiting for new data : ({err})"),
                };
            }
            _ = data.changed() => {
                warn!("There was new data available before all previous could be sent, abording push");
            }
        }
    }
}

pub async fn send_data_to_remote(
    name: &str,
    remote: Arc<Mutex<impl Remote + ?Sized>>,
    data: &HashMap<String, HashMap<String, RegisterValue>>,
) -> Result<(), RemoteError> {
    info!("Sending to remote {name}");
    for (source, values) in data.iter() {
        remote.lock().await.send_measurement(source, values).await?;
    }
    Ok(())
}
