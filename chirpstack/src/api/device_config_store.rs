use std::str::FromStr;

use tonic::{Request, Response, Status};
use uuid::Uuid;

use chirpstack_api::api;
use chirpstack_api::api::device_config_store_service_server::DeviceConfigStoreService;
use lrwn::EUI64;

use super::auth::validator;
use super::error::ToStatus;
use super::helpers;
use crate::region;
use crate::storage::{self, device_config_store};

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
        let req_dcs = match &request.get_ref().device_config_store {
            Some(v) => v,
            None => {
                return Err(Status::invalid_argument("device_config_store is missing"));
            }
        };

        let dev_eui = EUI64::from_str(&req_dcs.dev_eui).map_err(|e| e.status())?;

        self.validator
            .validate(
                request.extensions(),
                validator::ValidateDeviceConfigStoreAccess::new(validator::Flag::Update, dev_eui),
            )
            .await?;

        // upsert
        let _ = device_config_store::upsert(device_config_store::DeviceConfigStore {
            dev_eui,
            chmask_config: req_dcs.chmask_config.clone(),
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
                validator::ValidateDeviceConfigStoreAccess::new(validator::Flag::Read, dev_eui),
            )
            .await?;

        let dcs = device_config_store::get(&dev_eui)
            .await
            .map_err(|e| e.status())?;

        Ok(Response::new(api::GetDeviceConfigStoreResponse {
            device_config_store: Some(api::DeviceConfigStore {
                dev_eui: dcs.dev_eui.to_string(),
                chmask_config: dcs.chmask_config,
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
                validator::ValidateDeviceConfigStoreAccess::new(validator::Flag::Delete, dev_eui),
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
                validator::ValidateDeviceConfigStoresAccess::new(validator::Flag::List, app_id),
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

    async fn get_config_store_alignment(
        &self,
        request: Request<api::GetConfigStoreAlignmentRequest>,
    ) -> Result<Response<api::GetConfigStoreAlignmentResponse>, Status> {
        let req = request.get_ref();

        let dev_eui = EUI64::from_str(&req.dev_eui).map_err(|e| e.status())?;

        self.validator
            .validate(
                request.extensions(),
                validator::ValidateDeviceConfigStoreAccess::new(validator::Flag::Read, dev_eui),
            )
            .await?;

        Ok(Response::new(api::GetConfigStoreAlignmentResponse {
            alignment: Some(
                device_config_store::get_alignment(&dev_eui)
                    .await
                    .map_err(|e| e.status())?,
            ),
        }))
    }

    async fn get_available_uplink_channels(
        &self,
        request: Request<api::GetAvailableChannelsRequest>,
    ) -> Result<Response<api::GetAvailableChannelsResponse>, Status> {
        let req = request.get_ref();

        let dev_eui = EUI64::from_str(&req.dev_eui).map_err(|e| e.status())?;

        self.validator
            .validate(
                request.extensions(),
                validator::ValidateDeviceAccess::new(validator::Flag::Read, dev_eui),
            )
            .await?;

        let channels = {
            let d = storage::device::get(&dev_eui)
                .await
                .map_err(|e| e.status())?;

            let ds = d.get_device_session().map_err(|e| e.status())?;

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
                            min_dr: c.min_dr as u32,
                            max_dr: c.max_dr as u32,
                            enabled: enabled.contains(i),
                            user_defined: c.user_defined,
                        },
                    )
                })
                .collect()
        };

        Ok(Response::new(api::GetAvailableChannelsResponse {
            channels,
        }))
    }
}

#[cfg(test)]
pub mod test {
    use std::collections::HashMap;

    use super::*;
    use crate::api::auth;
    use crate::test;
    use chirpstack_api::internal;

    #[tokio::test]
    async fn test_device_config_store() {
        let _guard = test::prepare().await;

        // setup admin key
        let key = storage::api_key::test::create_api_key(true, false).await;

        // create device
        let d = {
            let dp = storage::device_profile::test::create_device_profile(None).await;
            let app = storage::application::test::create_application(Some(dp.tenant_id)).await;
            storage::device::create(storage::device::Device {
                name: "test-dev".into(),
                dev_eui: EUI64::from_be_bytes([1, 2, 3, 4, 5, 6, 7, 8]),
                application_id: app.id,
                device_profile_id: dp.id,
                device_session: Some(internal::DeviceSession {
                    region_config_id: "eu868".into(),
                    enabled_uplink_channel_indices: vec![0, 2],
                    ..Default::default()
                }),
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
                device_config_store: Some(api::DeviceConfigStore {
                    dev_eui: d.dev_eui.to_string(),
                    chmask_config: Some(api::ChMaskConfig {
                        enabled_uplink_channel_indices: vec![0, 2],
                    }),
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
                dev_eui: d.dev_eui.to_string(),
                chmask_config: Some(api::ChMaskConfig {
                    enabled_uplink_channel_indices: vec![0, 2],
                }),
            }),
            get_resp.get_ref().device_config_store
        );

        // update
        let update_req = get_request(
            &key.id,
            api::SetDeviceConfigStoreRequest {
                device_config_store: Some(api::DeviceConfigStore {
                    dev_eui: d.dev_eui.to_string(),
                    chmask_config: Some(api::ChMaskConfig {
                        enabled_uplink_channel_indices: vec![0, 1, 2],
                    }),
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
                dev_eui: d.dev_eui.to_string(),
                chmask_config: Some(api::ChMaskConfig {
                    enabled_uplink_channel_indices: vec![0, 1, 2],
                }),
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
            api::GetConfigStoreAlignmentRequest {
                dev_eui: d.dev_eui.to_string(),
            },
        );
        let align_resp = service.get_config_store_alignment(align_req).await.unwrap();
        assert_eq!(
            Some(api::ConfigStoreAlignment {
                chmask_config: false
            }),
            align_resp.get_ref().alignment
        );

        // get channels with correct enabled status
        let chan_req = get_request(
            &key.id,
            api::GetAvailableChannelsRequest {
                dev_eui: d.dev_eui.to_string(),
            },
        );
        let chan_resp = service
            .get_available_uplink_channels(chan_req)
            .await
            .unwrap();
        assert_eq!(
            HashMap::from([
                (
                    0,
                    api::DeviceUplinkChannel {
                        frequency: 868100000,
                        min_dr: 0,
                        max_dr: 5,
                        enabled: true,
                        user_defined: false
                    }
                ),
                (
                    1,
                    api::DeviceUplinkChannel {
                        frequency: 868300000,
                        min_dr: 0,
                        max_dr: 5,
                        enabled: false,
                        user_defined: false
                    }
                ),
                (
                    2,
                    api::DeviceUplinkChannel {
                        frequency: 868500000,
                        min_dr: 0,
                        max_dr: 5,
                        enabled: true,
                        user_defined: false
                    }
                ),
            ]),
            chan_resp.get_ref().channels
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
