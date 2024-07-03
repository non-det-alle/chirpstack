use std::collections::HashMap;

use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::RwLock;
use tracing::{info, trace, warn};

use crate::config;
use chirpstack_api::internal;
use lrwn::EUI64;

pub mod default;
pub mod plugin;

lazy_static! {
    static ref CHMASK_ALGORITHMS: RwLock<HashMap<String, Box<dyn Handler + Sync + Send>>> =
        RwLock::new(HashMap::new());
}

pub async fn setup() -> Result<()> {
    info!("Setting up channel mask algorithms");
    let mut algos = CHMASK_ALGORITHMS.write().await;

    trace!("Setting up included algorithms");
    let a = default::Algorithm::new();
    algos.insert(a.get_id(), Box::new(a));

    trace!("Setting up plugins");
    let conf = config::get();
    for file_path in &conf.network.chmask_plugins {
        info!(file_path = %file_path, "Setting up ChannelMask plugin");
        let a = plugin::Plugin::new(file_path)?;
        algos.insert(a.get_id(), Box::new(a));
    }

    Ok(())
}

pub async fn get_algorithms() -> HashMap<String, String> {
    let mut out: HashMap<String, String> = HashMap::new();

    let algos = CHMASK_ALGORITHMS.read().await;
    for (_, v) in algos.iter() {
        out.insert(v.get_id(), v.get_name());
    }

    out
}

pub async fn handle(algo_id: &str, req: &Request<'_>) -> Response {
    let algos = CHMASK_ALGORITHMS.read().await;
    match algos.get(algo_id) {
        Some(v) => match v.handle(req).await {
            Ok(v) => v,
            Err(e) => {
                warn!(algorithm_id = %algo_id, error = %e, "ChannelMask algorithm returned error");
                req.enabled_uplink_channel_indices.into()
            }
        },
        None => {
            warn!(algorithm_id = %algo_id, "No ChannelMask algorithm configured with given ID");
            req.enabled_uplink_channel_indices.into()
        }
    }
}

#[async_trait]
pub trait Handler {
    // Returns the name.
    fn get_name(&self) -> String;

    // Get the ID.
    fn get_id(&self) -> String;

    // Handle the ChannelMask request.
    async fn handle(&self, req: &Request) -> Result<Response>;
}

#[derive(Clone)]
pub struct Request<'a> {
    pub region_config_id: String,
    pub region_common_name: lrwn::region::CommonName,
    pub dev_eui: EUI64,
    pub mac_version: lrwn::region::MacVersion,
    pub reg_params_revision: lrwn::region::Revision,
    pub enabled_uplink_channel_indices: &'a [usize],
    pub provisioned_uplink_channel_indices: &'a [usize],
    pub uplink_history: Vec<internal::UplinkAdrHistory>,
    pub device_variables: HashMap<String, String>,
}

type Response = Vec<usize>;
