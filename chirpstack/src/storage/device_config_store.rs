use anyhow::Result;
use chrono::{DateTime, Utc};
use diesel::{dsl, prelude::*};
use diesel_async::RunQueryDsl;
use tracing::info;
use uuid::Uuid;

use lrwn::EUI64;

use super::schema::{device, device_config_store};
use super::{error::Error, fields, get_async_db_conn};

#[derive(Queryable, Insertable, AsChangeset, PartialEq, Debug, Clone)]
#[diesel(table_name = device_config_store)]
pub struct DeviceConfigStore {
    pub dev_eui: EUI64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub chmask_config: fields::ChMaskConfig,
    pub dr: Option<i16>,
    pub tx_power_index: Option<i16>,
    pub nb_trans: Option<i16>,
    pub max_duty_cycle: Option<i16>,
}

#[derive(PartialEq, Debug, Clone)]
pub struct ConfigStoreAlignment {
    pub enabled_uplink_channel_indices: Option<bool>,
    pub dr: Option<bool>,
    pub tx_power_index: Option<bool>,
    pub nb_trans: Option<bool>,
    pub max_duty_cycle: Option<bool>,
}

impl DeviceConfigStore {
    fn validate(&mut self) -> Result<(), Error> {
        // chain all configurations here with &&
        if self.chmask_config.is_empty()
            && self.dr.is_none()
            && self.tx_power_index.is_none()
            && self.nb_trans.is_none()
            && self.max_duty_cycle.is_none()
        {
            return Err(Error::Validation(
                "empty configuration, consider deleting".into(),
            ));
        }

        // chmask_config
        if !self.chmask_config.is_empty() {
            let uc = &mut self.chmask_config;
            // validate
            for &c in uc.iter() {
                if u8::try_from(c).is_err() {
                    return Err(Error::Validation(
                        "provided channel index is out-of-bounds".into(),
                    ));
                }
            }
            // format
            uc.sort_unstable();
            uc.dedup();
        }

        // dr
        if let Some(dr) = self.dr {
            // validate
            if u8::try_from(dr).is_err() {
                return Err(Error::Validation(
                    "provided dr value is out-of-bounds".into(),
                ));
            }
        }

        // tx_power_index
        if let Some(tx_power_index) = self.tx_power_index {
            // validate
            if u8::try_from(tx_power_index).is_err() {
                return Err(Error::Validation(
                    "provided tx_power_index value is out-of-bounds".into(),
                ));
            }
        }

        // nb_trans
        if let Some(nb_trans) = self.nb_trans {
            // validate
            if u8::try_from(nb_trans).is_err() {
                return Err(Error::Validation(
                    "provided nb_trans value is out-of-bounds".into(),
                ));
            }
        }

        // max_duty_cycle
        if let Some(max_duty_cycle) = self.max_duty_cycle {
            // validate
            if u8::try_from(max_duty_cycle).is_err() {
                return Err(Error::Validation(
                    "provided max_duty_cycle value is out-of-bounds".into(),
                ));
            }
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
            chmask_config: vec![].into(),
            dr: None,
            tx_power_index: None,
            nb_trans: None,
            max_duty_cycle: None,
        }
    }
}

#[derive(Queryable, PartialEq, Eq, Debug)]
pub struct DeviceConfigStoreListItem {
    pub dev_eui: EUI64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub async fn upsert(mut dcs: DeviceConfigStore) -> Result<DeviceConfigStore, Error> {
    dcs.validate()?;

    let dcs: DeviceConfigStore = diesel::insert_into(device_config_store::table)
        .values(&dcs)
        .on_conflict(device_config_store::dev_eui)
        .do_update()
        .set((
            device_config_store::updated_at.eq(Utc::now()),
            device_config_store::chmask_config.eq(&dcs.chmask_config),
            device_config_store::dr.eq(&dcs.dr),
            device_config_store::tx_power_index.eq(&dcs.tx_power_index),
            device_config_store::nb_trans.eq(&dcs.nb_trans),
            device_config_store::max_duty_cycle.eq(&dcs.max_duty_cycle),
        ))
        .get_result(&mut get_async_db_conn().await?)
        .await
        .map_err(|e| Error::from_diesel(e, dcs.dev_eui.to_string()))?;
    info!(dev_eui = %dcs.dev_eui, "Device config store set");
    Ok(dcs)
}

pub async fn get(dev_eui: &EUI64) -> Result<DeviceConfigStore, Error> {
    let dcs = device_config_store::table
        .find(&dev_eui)
        .first(&mut get_async_db_conn().await?)
        .await
        .map_err(|e| Error::from_diesel(e, dev_eui.to_string()))?;
    Ok(dcs)
}

pub async fn delete(dev_eui: &EUI64) -> Result<(), Error> {
    let ra = diesel::delete(device_config_store::table.find(&dev_eui))
        .execute(&mut get_async_db_conn().await?)
        .await?;
    if ra == 0 {
        return Err(Error::NotFound(dev_eui.to_string()));
    }
    info!(dev_eui = %dev_eui, "Device config store deleted");
    Ok(())
}

pub async fn get_count(application_id: &Option<Uuid>) -> Result<i64, Error> {
    let mut q = device_config_store::table
        .select(dsl::count_star())
        .distinct()
        .inner_join(device::table)
        .into_boxed();

    if let Some(application_id) = application_id {
        q = q.filter(device::application_id.eq(fields::Uuid::from(application_id)));
    }

    Ok(q.first(&mut get_async_db_conn().await?).await?)
}

pub async fn list(
    limit: i64,
    offset: i64,
    application_id: &Option<Uuid>,
) -> Result<Vec<DeviceConfigStoreListItem>, Error> {
    let mut q = device_config_store::table
        .inner_join(device::table)
        .select((
            device_config_store::dev_eui,
            device_config_store::created_at,
            device_config_store::updated_at,
        ))
        .distinct()
        .into_boxed();

    if let Some(application_id) = application_id {
        q = q.filter(device::application_id.eq(fields::Uuid::from(application_id)));
    }

    q.order_by(device_config_store::dev_eui)
        .limit(limit)
        .offset(offset)
        .load(&mut get_async_db_conn().await?)
        .await
        .map_err(|e| Error::from_diesel(e, "".into()))
}

pub async fn get_alignment(dev_eui: &EUI64) -> Result<ConfigStoreAlignment, Error> {
    let (dcs, ds): (DeviceConfigStore, Option<fields::DeviceSession>) = device_config_store::table
        .find(&dev_eui)
        .inner_join(device::table)
        .select((device_config_store::all_columns, device::device_session))
        .first(&mut get_async_db_conn().await?)
        .await
        .map_err(|e| Error::from_diesel(e, dev_eui.to_string()))?;

    let ds = ds.ok_or_else(|| Error::NotFound(dev_eui.to_string()))?;

    Ok(ConfigStoreAlignment {
        enabled_uplink_channel_indices: (!dcs.chmask_config.is_empty())
            .then(|| dcs.chmask_config == ds.enabled_uplink_channel_indices),
        dr: dcs.dr.map(|v| v == ds.dr as i16),
        tx_power_index: dcs.tx_power_index.map(|v| v == ds.tx_power_index as i16),
        nb_trans: dcs.nb_trans.map(|v| v == ds.nb_trans as i16),
        max_duty_cycle: dcs.max_duty_cycle.map(|v| v == ds.max_duty_cycle as i16),
    })
}

#[cfg(test)]
pub mod test {
    use super::*;
    use crate::storage::{application, device, device_profile};
    use crate::test;
    use chirpstack_api::internal;

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
        assert!(upsert(dcs).await.is_err());

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
                        enabled_uplink_channel_indices: vec![0, 1, 2],
                        dr: 0,
                        tx_power_index: 0,
                        nb_trans: 1,
                        max_duty_cycle: 0,
                        ..Default::default()
                    }
                    .into(),
                ),
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
        assert!(upsert(dcs).await.is_err());

        // not created yet
        assert!(get(&d.dev_eui).await.is_err());

        // create
        let mut dcs = upsert(
            DeviceConfigStore {
                dev_eui: d.dev_eui,
                chmask_config: vec![0, 1, 2].into(),
                dr: Some(1),
                tx_power_index: None,
                nb_trans: Some(8),
                max_duty_cycle: None,
                ..Default::default()
            }
            .into(),
        )
        .await
        .unwrap();

        // get
        let dcs_get = get(&d.dev_eui).await.unwrap();
        assert_eq!(dcs, dcs_get);

        // aligned
        let align = get_alignment(&d.dev_eui).await.unwrap();
        assert_eq!(
            ConfigStoreAlignment {
                enabled_uplink_channel_indices: Some(true),
                dr: Some(false),
                tx_power_index: None,
                nb_trans: Some(false),
                max_duty_cycle: None,
            },
            align
        );

        // update
        dcs.chmask_config = vec![0, 1, 2, 3].into();
        dcs.dr = None;
        dcs.tx_power_index = Some(0);
        dcs.nb_trans = None;
        dcs.max_duty_cycle = Some(14);
        dcs = upsert(dcs).await.unwrap();
        let dcs_get = get(&d.dev_eui).await.unwrap();
        assert_eq!(dcs, dcs_get);

        // re-check aligned
        let align = get_alignment(&d.dev_eui).await.unwrap();
        assert_eq!(
            ConfigStoreAlignment {
                enabled_uplink_channel_indices: Some(false),
                dr: None,
                tx_power_index: Some(true),
                nb_trans: None,
                max_duty_cycle: Some(false),
            },
            align
        );

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
                application_id: Some(d.application_id.into()),
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
