use std::{collections::HashMap, sync::Arc};

use log::{error, info, warn};
use tokio::{
    select,
    sync::{watch, Mutex},
    task::JoinSet,
};

use crate::types_conversion::RegisterValue;

pub mod remote;
use remote::{Remote, RemoteError};

pub mod errors;
pub mod influxdb;
pub mod prometheus;

/// Awaits and processes the completion of all remote sending tasks.
///
/// This helper function consumes results from a [`JoinSet`] of tasks,
/// where each task represents a push of measurement data to a remote
/// backend.  
/// 
/// Errors are logged but do not interrupt the processing of other tasks.
///
/// # Parameters
/// - `set`: The [`JoinSet`] containing remote sending tasks (`Result<(), RemoteError>`).
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

/// Continuously listens for new measurement data and pushes it to all configured remotes.
///
/// This function spawns a dedicated async task for each remote backend (InfluxDB,
/// Prometheus, etc.) whenever new data is available from the `watch::Receiver`.
/// 
/// It ensures that data is sent concurrently to all remotes, and handles cases where
/// new data arrives before the previous push finishes.
///
/// # Parameters
/// - `remotes`: A thread-safe shared map of remote backends (keyed by name),
///   each implementing the [`Remote`] trait.
/// - `data`: A [`watch::Receiver`] that broadcasts the latest measurement data,
///   structured as:
///   - Outer key = device/source name
///   - Inner map = field name → `RegisterValue`
pub async fn send_data_to_remotes(
    remotes: Arc<Mutex<HashMap<String, Arc<Mutex<Box<impl Remote + Send + 'static + ?Sized>>>>>>,
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


/// Sends collected register data to a configured remote backend.
///
/// This function iterates over all measurement sources and their
/// associated field values, and forwards them to the given `Remote`
/// implementation (e.g. InfluxDB, Prometheus).
///
/// # Parameters
/// - `name`: Logical name of the remote (used only for logging).
/// - `remote`: A thread-safe, asynchronous reference to a type
///   implementing the [`Remote`] trait.
/// - `data`: A nested map of measurements, where:
///   - Outer key = measurement source (e.g. device name).
///   - Inner map = field name → `RegisterValue`.
///
/// # Returns
/// - `Ok(())` if all measurements were successfully sent.
/// - `Err(RemoteError)` if sending failed.
pub async fn send_data_to_remote(
    name: &str,
    remote: Arc<Mutex<Box<impl Remote + ?Sized>>>,
    data: &HashMap<String, HashMap<String, RegisterValue>>,
) -> Result<(), RemoteError> {
    info!("Sending to remote {name}");
    for (source, values) in data.iter() {
        remote.lock().await.send_measurement(source, values).await?;
    }
    Ok(())
}
