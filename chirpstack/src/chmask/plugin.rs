use std::fs;

use anyhow::{Context, Result};
use async_trait::async_trait;
use rquickjs::CatchResultExt;

use super::{Handler, Request, Response};

pub struct Plugin {
    script: String,
    id: String,
    name: String,
}

impl Plugin {
    pub fn new(file_path: &str) -> Result<Self> {
        let rt = rquickjs::Runtime::new()?;
        let ctx = rquickjs::Context::full(&rt)?;
        let script = fs::read_to_string(file_path).context("Read ChannelMask plugin")?;

        let (id, name) = ctx.with::<_, Result<(String, String)>>(|ctx| {
            let m = rquickjs::Module::declare(ctx.clone(), "script", script.clone())
                .catch(&ctx)
                .map_err(|e| anyhow!("JS error: {}", e))
                .context("Declare script")?;

            let (m, m_promise) = m.eval().context("Evaluate script")?;
            m_promise.finish()?;
            let id_func: rquickjs::Function = m.get("id").context("Get id function")?;
            let name_func: rquickjs::Function = m.get("name").context("Get name function")?;

            let id: String = id_func.call(()).context("Call id function")?;
            let name: String = name_func.call(()).context("Call name function")?;

            Ok((id, name))
        })?;

        let p = Plugin { script, id, name };

        Ok(p)
    }
}

#[async_trait]
impl Handler for Plugin {
    fn get_name(&self) -> String {
        self.name.clone()
    }

    fn get_id(&self) -> String {
        self.id.clone()
    }

    async fn _handle(&self, req: &Request) -> Result<Response> {
        let rt = rquickjs::Runtime::new()?;
        let ctx = rquickjs::Context::full(&rt)?;

        ctx.with::<_, Result<Response>>(|ctx| {
            let m = rquickjs::Module::declare(ctx.clone(), "script", self.script.clone())
                .context("Declare script")?;
            let (m, m_promise) = m.eval().context("Evaluate script")?;
            m_promise.finish()?;
            let func: rquickjs::Function = m.get("handle").context("Get handle function")?;

            let input = rquickjs::Object::new(ctx.clone())?;
            input.set("regionConfigId", req.region_config_id.clone())?;
            input.set("regionCommonName", req.region_common_name.to_string())?;
            input.set("devEui", req.dev_eui.to_string())?;
            input.set("macVersion", req.mac_version.to_string())?;
            input.set("regParamsRevision", req.reg_params_revision.to_string())?;

            let uplink_channels = rquickjs::Object::new(ctx.clone())?;
            for (k, v) in &req.uplink_channels {
                let obj = rquickjs::Object::new(ctx.clone())?;
                obj.set("frequency", v.frequency)?;
                obj.set("minDr", v.min_dr)?;
                obj.set("maxDr", v.max_dr)?;
                obj.set("enabled", v.enabled)?;
                obj.set("userDefined", v.user_defined)?;
                uplink_channels.set(*k as u32, obj)?;
            }
            input.set("uplinkChannels", uplink_channels)?;

            let mut uplink_history: Vec<rquickjs::Object> = Vec::new();
            for uh in &req.uplink_history {
                let obj = rquickjs::Object::new(ctx.clone())?;
                obj.set("fCnt", uh.f_cnt)?;
                obj.set("maxSnr", uh.max_snr)?;
                obj.set("maxRssi", uh.max_rssi)?;
                obj.set("txPowerIndex", uh.tx_power_index)?;
                obj.set("gatewayCount", uh.gateway_count)?;
                uplink_history.push(obj);
            }
            input.set("uplinkHistory", uplink_history)?;

            let device_variables = rquickjs::Object::new(ctx.clone())?;
            for (k, v) in &req.device_variables {
                device_variables.set(k, v)?;
            }
            input.set("deviceVariables", device_variables)?;

            let res: Vec<usize> = func
                .call((input,))
                .catch(&ctx)
                .map_err(|e| anyhow!("JS error: {}", e))
                .context("Call handle function")?;

            Ok(Response(res))
        })
    }
}

#[cfg(test)]
pub mod test {
    use super::*;
    use lrwn::{
        region::{eu868::Configuration, Region},
        EUI64,
    };

    #[tokio::test]
    async fn test_plugin() {
        let p = Plugin::new("../examples/chmask_plugins/plugin_skeleton.js").unwrap();

        assert_eq!("Example plugin", p.get_name());
        assert_eq!("example_id", p.get_id());

        let mut c = Configuration::new(false);
        c.add_channel(867100000, 0, 5).unwrap();
        c.add_channel(867300000, 0, 5).unwrap();
        c.add_channel(867500000, 0, 5).unwrap();
        c.add_channel(867700000, 0, 5).unwrap();
        c.add_channel(867900000, 0, 5).unwrap();

        let uplink_channels = c.get_device_uplink_channels(
            &c.get_user_defined_uplink_channel_indices(),
            &c.get_enabled_uplink_channel_indices(),
        );

        let req = Request {
            region_config_id: "eu868".into(),
            region_common_name: lrwn::region::CommonName::EU868,
            dev_eui: EUI64::from_be_bytes([1, 2, 3, 4, 5, 6, 7, 8]),
            mac_version: lrwn::region::MacVersion::LORAWAN_1_0_3,
            reg_params_revision: lrwn::region::Revision::A,
            uplink_channels,
            uplink_history: vec![],
            device_variables: Default::default(),
        };

        let Response(resp) = p.handle(&req).await.unwrap();
        assert_eq!(vec![0, 1, 2, 3, 4, 5, 6, 7], resp);
    }
}
