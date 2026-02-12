use std::str::FromStr;

use uuid::Uuid;

use chirpstack_api::api;
use chirpstack_api::api::device_config_store_service_server::DeviceConfigStoreService;
use chirpstack_api::tonic::{self, Code, Request, Response, Status};
use lrwn::EUI64;

use super::auth::validator;
use super::error::ToStatus;
use super::helpers;
use crate::region;
use crate::storage::{device, device_config_store};

pub struct DeviceConfigStore {
    validator: validator::RequestValidator,
}

impl DeviceConfigStore {
    pub fn new(validator: validator::RequestValidator) -> Self {
        DeviceConfigStore { validator }
    }
}

#[tonic::async_trait]
impl DeviceConfigStoreService for DeviceConfigStore {
    async fn set(
        &self,
        request: Request<api::SetDeviceConfigStoreRequest>,
    ) -> Result<Response<()>, Status> {
        let req = request.get_ref();

        let dev_eui = EUI64::from_str(&req.dev_eui).map_err(|e| e.status())?;

        self.validator
            .validate(
                request.extensions(),
                validator::ValidateDeviceAccess::new(validator::Flag::Update, dev_eui),
            )
            .await?;

        let Some(dcs) = &req.device_config_store else {
            return Err(Status::invalid_argument("device_config_store not provided"));
        };

        // upsert
        let _ = device_config_store::upsert(device_config_store::DeviceConfigStore {
            dev_eui,
            chmask_config: dcs.enabled_uplink_channel_indices.clone().into(),
            dr: dcs.dr.map(|v| v as i16),
            tx_power_index: dcs.tx_power_index.map(|v| v as i16),
            nb_trans: dcs.nb_trans.map(|v| v as i16),
            max_duty_cycle: dcs.max_duty_cycle.map(|v| v as i16),
            ..Default::default()
        })
        .await
        .map_err(|e| e.status())?;

        Ok(Response::new(()))
    }

    async fn get(
        &self,
        request: Request<api::GetDeviceConfigStoreRequest>,
    ) -> Result<Response<api::GetDeviceConfigStoreResponse>, Status> {
        let req = request.get_ref();
        let dev_eui = EUI64::from_str(&req.dev_eui).map_err(|e| e.status())?;

        self.validator
            .validate(
                request.extensions(),
                validator::ValidateDeviceAccess::new(validator::Flag::Read, dev_eui),
            )
            .await?;

        let dcs = device_config_store::get(&dev_eui)
            .await
            .map_err(|e| e.status())?;

        Ok(Response::new(api::GetDeviceConfigStoreResponse {
            device_config_store: Some(api::DeviceConfigStore {
                enabled_uplink_channel_indices: dcs.chmask_config.into(),
                dr: dcs.dr.map(|v| v as u32),
                tx_power_index: dcs.tx_power_index.map(|v| v as u32),
                nb_trans: dcs.nb_trans.map(|v| v as u32),
                max_duty_cycle: dcs.max_duty_cycle.map(|v| v as u32),
            }),
            created_at: Some(helpers::datetime_to_prost_timestamp(&dcs.created_at)),
            updated_at: Some(helpers::datetime_to_prost_timestamp(&dcs.updated_at)),
        }))
    }

    async fn delete(
        &self,
        request: Request<api::DeleteDeviceConfigStoreRequest>,
    ) -> Result<Response<()>, Status> {
        let req = request.get_ref();
        let dev_eui = EUI64::from_str(&req.dev_eui).map_err(|e| e.status())?;

        self.validator
            .validate(
                request.extensions(),
                validator::ValidateDeviceAccess::new(validator::Flag::Delete, dev_eui),
            )
            .await?;

        device_config_store::delete(&dev_eui)
            .await
            .map_err(|e| e.status())?;

        Ok(Response::new(()))
    }

    async fn list(
        &self,
        request: Request<api::ListDeviceConfigStoresRequest>,
    ) -> Result<Response<api::ListDeviceConfigStoresResponse>, Status> {
        let req = request.get_ref();
        let app_id = Uuid::from_str(&req.application_id).map_err(|e| e.status())?;

        self.validator
            .validate(
                request.extensions(),
                validator::ValidateDevicesAccess::new(validator::Flag::List, app_id),
            )
            .await?;

        let count = device_config_store::get_count(&Some(app_id))
            .await
            .map_err(|e| e.status())?;
        let items = device_config_store::list(req.limit as i64, req.offset as i64, &Some(app_id))
            .await
            .map_err(|e| e.status())?;

        Ok(Response::new(api::ListDeviceConfigStoresResponse {
            total_count: count as u32,
            result: items
                .iter()
                .map(|dcs| api::DeviceConfigStoreListItem {
                    dev_eui: dcs.dev_eui.to_string(),
                    created_at: Some(helpers::datetime_to_prost_timestamp(&dcs.created_at)),
                    updated_at: Some(helpers::datetime_to_prost_timestamp(&dcs.updated_at)),
                })
                .collect(),
        }))
    }

    async fn get_device_config_alignment(
        &self,
        request: Request<api::GetDeviceConfigAlignmentRequest>,
    ) -> Result<Response<api::GetDeviceConfigAlignmentResponse>, Status> {
        let req = request.get_ref();
        let dev_eui = EUI64::from_str(&req.dev_eui).map_err(|e| e.status())?;

        self.validator
            .validate(
                request.extensions(),
                validator::ValidateDeviceAccess::new(validator::Flag::Read, dev_eui),
            )
            .await?;

        let dcs =
            device_config_store::get(&dev_eui)
                .await
                .map_err(|e| match e.status().code() {
                    Code::NotFound => {
                        Status::not_found(format!("Device config store not found (id: {dev_eui})"))
                    }
                    _ => e.status(),
                })?;

        let d = device::get(&dev_eui)
            .await
            .map_err(|e| match e.status().code() {
                Code::NotFound => {
                    Status::not_found(format!("Device does not exist (id: {dev_eui})"))
                }
                _ => e.status(),
            })?;

        let ds = d.device_session.ok_or_else(|| {
            Status::failed_precondition(format!("Device not (yet) activated (id: {dev_eui})"))
        })?;

        if ds.enabled_uplink_channel_indices.is_empty() {
            return Err(Status::unavailable(format!(
                "Device not (yet) seen (id: {dev_eui})"
            )));
        }

        Ok(Response::new(api::GetDeviceConfigAlignmentResponse {
            enabled_uplink_channel_indices: (!dcs.chmask_config.is_empty())
                .then(|| dcs.chmask_config == ds.enabled_uplink_channel_indices),
            dr: dcs.dr.map(|v| v == ds.dr as i16),
            tx_power_index: dcs.tx_power_index.map(|v| v == ds.tx_power_index as i16),
            nb_trans: dcs.nb_trans.map(|v| v == ds.nb_trans as i16),
            max_duty_cycle: dcs.max_duty_cycle.map(|v| v == ds.max_duty_cycle as i16),
        }))
    }

    async fn get_device_current_params(
        &self,
        request: Request<api::GetDeviceCurrentParamsRequest>,
    ) -> Result<Response<api::GetDeviceCurrentParamsResponse>, Status> {
        let req = request.get_ref();
        let dev_eui = EUI64::from_str(&req.dev_eui).map_err(|e| e.status())?;

        self.validator
            .validate(
                request.extensions(),
                validator::ValidateDeviceAccess::new(validator::Flag::Read, dev_eui),
            )
            .await?;

        let d = device::get(&dev_eui).await.map_err(|e| e.status())?;

        let ds = d.device_session.ok_or_else(|| {
            Status::failed_precondition(format!("Device not (yet) activated (id: {dev_eui})"))
        })?;

        if ds.enabled_uplink_channel_indices.is_empty() {
            return Err(Status::unavailable(format!(
                "Device not (yet) seen (id: {dev_eui})"
            )));
        }

        let channels = {
            let extra: Vec<usize> = ds
                .extra_uplink_channels
                .keys()
                .map(|i| *i as usize)
                .collect();

            let enabled: Vec<usize> = ds
                .enabled_uplink_channel_indices
                .iter()
                .map(|i| *i as usize)
                .collect();

            let r = region::get(&ds.region_config_id).map_err(|e| e.status())?;

            r.get_device_uplink_channel_indices(&extra)
                .iter()
                .map(|i| {
                    let c = r.get_uplink_channel(*i).unwrap();
                    (
                        *i as u32,
                        api::DeviceUplinkChannel {
                            frequency: c.frequency,
                            data_rates: c.data_rates.iter().clone().map(|&v| v as u32).collect(),
                            enabled: enabled.contains(i),
                            user_defined: c.user_defined,
                        },
                    )
                })
                .collect()
        };

        Ok(Response::new(api::GetDeviceCurrentParamsResponse {
            channels,
            dr: ds.dr,
            tx_power_index: ds.tx_power_index,
            nb_trans: ds.nb_trans,
            max_duty_cycle: ds.max_duty_cycle,
        }))
    }
}

#[cfg(test)]
pub mod test {
    use std::collections::HashMap;

    use super::*;
    use crate::api::auth;
    use crate::storage::{api_key, application, device_profile};
    use crate::test;
    use chirpstack_api::internal;

    #[tokio::test]
    async fn test_device_config_store() {
        let _guard = test::prepare().await;

        // setup admin key
        let key = api_key::test::create_api_key(true, false).await;

        // create device
        let d = {
            let dp = device_profile::test::create_device_profile(None).await;
            let app = application::test::create_application(None).await;
            device::create(device::Device {
                name: "test-dev".into(),
                dev_eui: EUI64::from_be_bytes([1, 2, 3, 4, 5, 6, 7, 8]),
                application_id: app.id,
                device_profile_id: dp.id,
                device_session: Some(
                    internal::DeviceSession {
                        region_config_id: "eu868".into(),
                        enabled_uplink_channel_indices: vec![0, 2],
                        dr: 3,
                        tx_power_index: 0,
                        nb_trans: 2,
                        max_duty_cycle: 6,
                        ..Default::default()
                    }
                    .into(),
                ),
                ..Default::default()
            })
        }
        .await
        .unwrap();

        // setup the api
        let service = DeviceConfigStore::new(validator::RequestValidator::new());

        // create
        let create_req = get_request(
            &key.id,
            api::SetDeviceConfigStoreRequest {
                dev_eui: d.dev_eui.to_string(),
                device_config_store: Some(api::DeviceConfigStore {
                    enabled_uplink_channel_indices: vec![0, 2],
                    dr: Some(3),
                    tx_power_index: Some(6),
                    nb_trans: Some(2),
                    max_duty_cycle: None,
                }),
            },
        );
        let _ = service.set(create_req).await.unwrap();

        // get
        let get_req = get_request(
            &key.id,
            api::GetDeviceConfigStoreRequest {
                dev_eui: d.dev_eui.to_string(),
            },
        );
        let get_resp = service.get(get_req).await.unwrap();
        assert_eq!(
            Some(api::DeviceConfigStore {
                enabled_uplink_channel_indices: vec![0, 2],
                dr: Some(3),
                tx_power_index: Some(6),
                nb_trans: Some(2),
                max_duty_cycle: None,
            }),
            get_resp.get_ref().device_config_store
        );

        // update
        let update_req = get_request(
            &key.id,
            api::SetDeviceConfigStoreRequest {
                dev_eui: d.dev_eui.to_string(),
                device_config_store: Some(api::DeviceConfigStore {
                    enabled_uplink_channel_indices: vec![0, 1, 2],
                    dr: Some(5),
                    tx_power_index: Some(0),
                    nb_trans: Some(1),
                    max_duty_cycle: Some(6),
                }),
            },
        );
        let _ = service.set(update_req).await.unwrap();

        // get updated
        let get_req = get_request(
            &key.id,
            api::GetDeviceConfigStoreRequest {
                dev_eui: d.dev_eui.to_string(),
            },
        );
        let get_resp = service.get(get_req).await.unwrap();
        assert_eq!(
            Some(api::DeviceConfigStore {
                enabled_uplink_channel_indices: vec![0, 1, 2],
                dr: Some(5),
                tx_power_index: Some(0),
                nb_trans: Some(1),
                max_duty_cycle: Some(6),
            }),
            get_resp.get_ref().device_config_store
        );

        // list
        let list_req = get_request(
            &key.id,
            api::ListDeviceConfigStoresRequest {
                application_id: d.application_id.to_string(),
                limit: 10,
                offset: 0,
            },
        );
        let list_resp = service.list(list_req).await.unwrap();
        assert_eq!(1, list_resp.get_ref().total_count);
        assert_eq!(1, list_resp.get_ref().result.len());

        // get alignment
        let align_req = get_request(
            &key.id,
            api::GetDeviceConfigAlignmentRequest {
                dev_eui: d.dev_eui.to_string(),
            },
        );
        let align_resp = service
            .get_device_config_alignment(align_req)
            .await
            .unwrap();
        assert_eq!(
            api::GetDeviceConfigAlignmentResponse {
                enabled_uplink_channel_indices: Some(false),
                dr: Some(false),
                tx_power_index: Some(true),
                nb_trans: Some(false),
                max_duty_cycle: Some(true),
            },
            align_resp.into_inner()
        );

        // get current params of the device
        let param_req = get_request(
            &key.id,
            api::GetDeviceCurrentParamsRequest {
                dev_eui: d.dev_eui.to_string(),
            },
        );
        let param_resp = service.get_device_current_params(param_req).await.unwrap();
        assert_eq!(
            api::GetDeviceCurrentParamsResponse {
                channels: HashMap::from([
                    (
                        0,
                        api::DeviceUplinkChannel {
                            frequency: 868100000,
                            data_rates: vec![0, 1, 2, 3, 4, 5],
                            enabled: true,
                            user_defined: false
                        }
                    ),
                    (
                        1,
                        api::DeviceUplinkChannel {
                            frequency: 868300000,
                            data_rates: vec![0, 1, 2, 3, 4, 5],
                            enabled: false,
                            user_defined: false
                        }
                    ),
                    (
                        2,
                        api::DeviceUplinkChannel {
                            frequency: 868500000,
                            data_rates: vec![0, 1, 2, 3, 4, 5],
                            enabled: true,
                            user_defined: false
                        }
                    ),
                ]),
                dr: 3,
                tx_power_index: 0,
                nb_trans: 2,
                max_duty_cycle: 6,
            },
            param_resp.into_inner()
        );

        // delete
        let del_req = get_request(
            &key.id,
            api::DeleteDeviceConfigStoreRequest {
                dev_eui: d.dev_eui.to_string(),
            },
        );
        let _ = service.delete(del_req).await.unwrap();

        let del_req = get_request(
            &key.id,
            api::DeleteDeviceConfigStoreRequest {
                dev_eui: d.dev_eui.to_string(),
            },
        );
        let del_resp = service.delete(del_req).await;
        assert!(del_resp.is_err());
    }

    fn get_request<T>(api_key_id: &Uuid, req: T) -> Request<T> {
        let mut req = Request::new(req);
        req.extensions_mut().insert(auth::AuthID::Key(*api_key_id));
        req
    }
}
