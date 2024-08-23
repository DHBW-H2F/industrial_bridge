use std::{cell::RefCell, collections::HashMap, rc::Rc, sync::Arc};

use industrial_device::IndustrialDevice;
use log::info;
use tokio::{sync::Mutex, task::JoinSet};

// Connect all devices passed as arguments to their targets, panics on error (this should then only be used in the initialisation)
// The connection for all devices is realized in parallel
pub async fn connect_devices<T: IndustrialDevice + Send + 'static>(
    devices: Rc<RefCell<HashMap<String, Arc<Mutex<T>>>>>,
) {
    // Create a task for each target
    let mut set = JoinSet::new();
    for (name, device) in devices.borrow().iter() {
        let d = device.clone();
        let name = name.clone();
        set.spawn(async move {
            let dc = d.clone();
            let mut dm = dc.lock().await;
            let res = dm.connect().await;
            (name, res)
        });
    }

    // Wait for completion
    async {
        while let Some(res) = set.join_next().await {
            match res {
                Ok((name, res)) => match res {
                    Ok(_) => info!("Connected to {name}"),
                    Err(err) => panic!("Could not connect to {name} ({err})"),
                },
                Err(err) => panic!("Error while joining connection threads ({err})"),
            }
        }
    }
    .await;
}
