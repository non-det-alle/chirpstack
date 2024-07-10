use anyhow::Result;
use async_trait::async_trait;

use super::{Handler, Request, Response};

pub struct Algorithm {}

impl Algorithm {
    pub fn new() -> Self {
        Algorithm {}
    }
}

#[async_trait]
impl Handler for Algorithm {
    fn get_name(&self) -> String {
        "Default behaviour (do nothing)".to_string()
    }

    fn get_id(&self) -> String {
        "default".to_string()
    }

    async fn handle(&self, req: &Request) -> Result<Response> {
        Ok(req.dry_response())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::test;
    use std::str::FromStr;

    #[test]
    fn test_id() {
        let a = Algorithm::new();
        assert_eq!("default", a.get_id());
    }

    #[tokio::test]
    async fn test_handle() {
        let a = Algorithm::new();
        let _guard = test::prepare().await;

        let c = lrwn::region::Channel {
            enabled: true,
            ..Default::default()
        };

        let req = Request {
            region_config_id: "eu868".into(),
            region_common_name: lrwn::region::CommonName::EU868,
            dev_eui: lrwn::EUI64::from_str("0102030405060708").unwrap(),
            mac_version: lrwn::region::MacVersion::LORAWAN_1_0_4,
            reg_params_revision: lrwn::region::Revision::RP002_1_0_3,
            uplink_channels: std::collections::HashMap::from([
                (0, c.clone()),
                (1, c.clone()),
                (2, c.clone()),
                (3, Default::default()),
                (4, Default::default()),
            ]),
            uplink_history: vec![],
            device_variables: Default::default(),
        };

        let resp = a.handle(&req).await.unwrap();
        assert_eq!(Response(vec![0, 1, 2]), resp);
    }
}
