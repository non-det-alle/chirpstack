use anyhow::Result;
use chrono::{DateTime, Utc};
use diesel::{dsl, prelude::*};
use diesel_async::RunQueryDsl;
use tracing::info;
use uuid::Uuid;

use chirpstack_api::{api, internal};
use lrwn::EUI64;

use super::error::Error;
use super::get_async_db_conn;
use super::schema::{device, device_config_store};

#[derive(Queryable, Insertable, AsChangeset, PartialEq, Debug, Clone)]
#[diesel(table_name = device_config_store)]
pub struct DeviceConfigStore {
    pub dev_eui: EUI64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub chmask_config: Option<api::ChMaskConfig>,
}

impl DeviceConfigStore {
    fn validate(&mut self) -> Result<(), Error> {
        // chain all configurations here with ||
        if self.chmask_config.is_none() {
            return Err(Error::Validation(
                "empty configuration, consider deleting".into(),
            ));
        }

        // chmask_config
        if let Some(cm) = self.chmask_config.as_mut() {
            let uc = &mut cm.enabled_uplink_channel_indices;
            // validate
            if uc.is_empty() {
                return Err(Error::Validation("provided chmask_config is empty".into()));
            }
            // format
            uc.sort_unstable();
            uc.dedup();
        }

        Ok(())
    }
}

impl Default for DeviceConfigStore {
    fn default() -> Self {
        let now = Utc::now();

        DeviceConfigStore {
            dev_eui: EUI64::from_be_bytes([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]),
            created_at: now,
            updated_at: now,
            chmask_config: None,
        }
    }
}

#[derive(PartialEq, Debug)]
pub struct DeviceConfigStoreListItem {
    pub dev_eui: EUI64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub alignment: Option<api::ConfigStoreAlignment>,
}

pub async fn create(mut dcs: DeviceConfigStore) -> Result<DeviceConfigStore, Error> {
    dcs.validate()?;

    let dcs: DeviceConfigStore = diesel::insert_into(device_config_store::table)
        .values(&dcs)
        .get_result(&mut get_async_db_conn().await?)
        .await
        .map_err(|e| Error::from_diesel(e, dcs.dev_eui.to_string()))?;
    info!(
        dev_eui = %dcs.dev_eui,
        "Device config store created"
    );
    Ok(dcs)
}

pub async fn get(dev_eui: &EUI64) -> Result<DeviceConfigStore, Error> {
    let dcs = device_config_store::dsl::device_config_store
        .find(&dev_eui)
        .first(&mut get_async_db_conn().await?)
        .await
        .map_err(|e| Error::from_diesel(e, dev_eui.to_string()))?;
    Ok(dcs)
}

pub async fn update(mut dcs: DeviceConfigStore) -> Result<DeviceConfigStore, Error> {
    dcs.validate()?;

    let dcs: DeviceConfigStore =
        diesel::update(device_config_store::dsl::device_config_store.find(&dcs.dev_eui))
            .set((
                device_config_store::updated_at.eq(Utc::now()),
                device_config_store::chmask_config.eq(&dcs.chmask_config),
            ))
            .get_result(&mut get_async_db_conn().await?)
            .await
            .map_err(|e| Error::from_diesel(e, dcs.dev_eui.to_string()))?;
    info!(dev_eui = %dcs.dev_eui, "Device config store updated");
    Ok(dcs)
}

pub async fn delete(dev_eui: &EUI64) -> Result<(), Error> {
    let ra = diesel::delete(device_config_store::dsl::device_config_store.find(&dev_eui))
        .execute(&mut get_async_db_conn().await?)
        .await?;
    if ra == 0 {
        return Err(Error::NotFound(dev_eui.to_string()));
    }
    info!(dev_eui = %dev_eui, "Device config store deleted");
    Ok(())
}

pub async fn get_count(application_id: &Option<Uuid>) -> Result<i64, Error> {
    let mut q = device_config_store::dsl::device_config_store
        .select(dsl::count_star())
        .distinct()
        .inner_join(device::table)
        .into_boxed();

    if let Some(application_id) = application_id {
        q = q.filter(device::dsl::application_id.eq(application_id));
    }

    Ok(q.first(&mut get_async_db_conn().await?).await?)
}

#[derive(Queryable)]
struct DeviceConfigStoreSession {
    device_config_store: DeviceConfigStore,
    device_session: Option<internal::DeviceSession>,
}

pub async fn list(
    limit: i64,
    offset: i64,
    application_id: &Option<Uuid>,
) -> Result<Vec<DeviceConfigStoreListItem>, Error> {
    let mut q = device_config_store::dsl::device_config_store
        .inner_join(device::table)
        .select((
            device_config_store::all_columns,
            device::dsl::device_session,
        ))
        .distinct()
        .into_boxed();

    if let Some(application_id) = application_id {
        q = q.filter(device::dsl::application_id.eq(application_id));
    }

    let dcs_dss: Vec<DeviceConfigStoreSession> = q
        .limit(limit)
        .offset(offset)
        .load(&mut get_async_db_conn().await?)
        .await
        .map_err(|e| Error::from_diesel(e, "".into()))?;

    let items = dcs_dss
        .iter()
        .map(|dcs_ds| DeviceConfigStoreListItem {
            dev_eui: dcs_ds.device_config_store.dev_eui,
            created_at: dcs_ds.device_config_store.created_at,
            updated_at: dcs_ds.device_config_store.updated_at,
            alignment: dcs_ds
                .device_session
                .as_ref()
                .map(|ds| api::ConfigStoreAlignment {
                    chmask_config: match dcs_ds.device_config_store.chmask_config.as_ref() {
                        Some(cm) => {
                            cm.enabled_uplink_channel_indices == ds.enabled_uplink_channel_indices
                        }
                        None => true,
                    },
                }),
        })
        .collect();

    Ok(items)
}

pub async fn get_alignment(dev_eui: &EUI64) -> Result<Option<api::ConfigStoreAlignment>, Error> {
    let dcs_ds: DeviceConfigStoreSession = device_config_store::dsl::device_config_store
        .find(&dev_eui)
        .inner_join(device::table)
        .select((
            device_config_store::all_columns,
            device::dsl::device_session,
        ))
        .first(&mut get_async_db_conn().await?)
        .await
        .map_err(|e| Error::from_diesel(e, dev_eui.to_string()))?;

    let alignment = dcs_ds.device_session.map(|ds| api::ConfigStoreAlignment {
        chmask_config: match dcs_ds.device_config_store.chmask_config {
            Some(cm) => cm.enabled_uplink_channel_indices == ds.enabled_uplink_channel_indices,
            None => true,
        },
    });

    Ok(alignment)
}

#[cfg(test)]
pub mod test {
    use super::*;
    use crate::storage;
    use crate::test;

    struct FilterTest<'a> {
        application_id: Option<Uuid>,
        dcss: Vec<&'a DeviceConfigStore>,
        count: usize,
        limit: i64,
        offset: i64,
    }

    #[tokio::test]
    async fn test_device_config_store() {
        let _guard = test::prepare().await;

        // device does not exist
        let dcs = DeviceConfigStore {
            dev_eui: EUI64::from_be_bytes([1, 2, 3, 4, 5, 6, 7, 8]),
            ..Default::default()
        };
        assert!(create(dcs).await.is_err());

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
                    enabled_uplink_channel_indices: vec![0, 1, 2],
                    ..Default::default()
                }),
                ..Default::default()
            })
        }
        .await
        .unwrap();

        // invalid empty config store
        let dcs = DeviceConfigStore {
            dev_eui: d.dev_eui,
            ..Default::default()
        };
        assert!(create(dcs).await.is_err());

        // invalid empty channel mask vector
        let dcs = DeviceConfigStore {
            dev_eui: d.dev_eui,
            chmask_config: Some(api::ChMaskConfig {
                enabled_uplink_channel_indices: vec![],
            }),
            ..Default::default()
        };
        assert!(create(dcs).await.is_err());

        // not created yet
        assert!(get(&d.dev_eui).await.is_err());

        // create
        let mut dcs = create(DeviceConfigStore {
            dev_eui: d.dev_eui,
            chmask_config: Some(api::ChMaskConfig {
                enabled_uplink_channel_indices: vec![0, 1, 2],
            }),
            ..Default::default()
        })
        .await
        .unwrap();

        // get
        let dcs_get = get(&d.dev_eui).await.unwrap();
        assert_eq!(dcs, dcs_get);

        // aligned
        let align = get_alignment(&d.dev_eui).await.unwrap().unwrap();
        assert!(align.chmask_config);

        // update
        dcs.chmask_config = Some(api::ChMaskConfig {
            enabled_uplink_channel_indices: vec![0, 1, 2, 3],
        });
        dcs = update(dcs).await.unwrap();
        let dcs_get = get(&d.dev_eui).await.unwrap();
        assert_eq!(dcs, dcs_get);

        // not aligned
        let align = get_alignment(&d.dev_eui).await.unwrap().unwrap();
        assert!(!align.chmask_config);

        // get count and list
        let tests = vec![
            FilterTest {
                application_id: None,
                dcss: vec![&dcs],
                count: 1,
                limit: 10,
                offset: 0,
            },
            FilterTest {
                application_id: Some(d.application_id),
                dcss: vec![&dcs],
                count: 1,
                limit: 10,
                offset: 0,
            },
            FilterTest {
                application_id: Some(Uuid::new_v4()),
                dcss: vec![],
                count: 0,
                limit: 10,
                offset: 0,
            },
        ];

        for tst in tests {
            let count = get_count(&tst.application_id).await.unwrap() as usize;
            assert_eq!(tst.count, count);

            let items = list(tst.limit, tst.offset, &tst.application_id)
                .await
                .unwrap();
            assert_eq!(
                tst.dcss
                    .iter()
                    .map(|dcs| dcs.dev_eui.to_string())
                    .collect::<String>(),
                items
                    .iter()
                    .map(|dcs| dcs.dev_eui.to_string())
                    .collect::<String>()
            );
        }

        // delete
        delete(&d.dev_eui).await.unwrap();
        assert!(delete(&d.dev_eui).await.is_err());
    }
}
