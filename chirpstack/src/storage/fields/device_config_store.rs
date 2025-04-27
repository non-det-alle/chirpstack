use std::io::Cursor;
use std::ops::{Deref, DerefMut};

use diesel::backend::Backend;
#[cfg(feature = "postgres")]
use diesel::pg::Pg;
use diesel::sql_types::Binary;
#[cfg(feature = "sqlite")]
use diesel::sqlite::Sqlite;
use diesel::{deserialize, serialize};
use prost::Message;

use chirpstack_api::api;

#[derive(Debug, Clone, PartialEq, AsExpression, FromSqlRow)]
#[diesel(sql_type = diesel::sql_types::Binary)]
pub struct ChMaskConfig(api::ChMaskConfig);

impl ChMaskConfig {
    pub fn new(m: api::ChMaskConfig) -> Self {
        ChMaskConfig(m)
    }
}

impl std::convert::From<api::ChMaskConfig> for ChMaskConfig {
    fn from(u: api::ChMaskConfig) -> Self {
        Self(u)
    }
}

impl std::convert::From<&api::ChMaskConfig> for ChMaskConfig {
    fn from(u: &api::ChMaskConfig) -> Self {
        Self::from(u.clone())
    }
}

impl std::convert::From<ChMaskConfig> for api::ChMaskConfig {
    fn from(val: ChMaskConfig) -> Self {
        val.0
    }
}

impl Deref for ChMaskConfig {
    type Target = api::ChMaskConfig;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ChMaskConfig {
    fn deref_mut(&mut self) -> &mut api::ChMaskConfig {
        &mut self.0
    }
}

impl<DB> deserialize::FromSql<Binary, DB> for ChMaskConfig
where
    DB: Backend,
    *const [u8]: deserialize::FromSql<Binary, DB>,
{
    fn from_sql(value: DB::RawValue<'_>) -> deserialize::Result<Self> {
        let bindata = <*const [u8] as deserialize::FromSql<Binary, DB>>::from_sql(value)?;
        let cm = api::ChMaskConfig::decode(Cursor::new(unsafe { &*bindata }))?;
        Ok(ChMaskConfig(cm))
    }
}

#[cfg(feature = "postgres")]
impl serialize::ToSql<Binary, Pg> for ChMaskConfig {
    fn to_sql<'b>(&'b self, out: &mut serialize::Output<'b, '_, Pg>) -> serialize::Result {
        let encoded = &self.encode_to_vec();
        <Vec<u8> as serialize::ToSql<Binary, Pg>>::to_sql(encoded, &mut out.reborrow())
    }
}

#[cfg(feature = "sqlite")]
impl serialize::ToSql<Binary, Sqlite> for ChMaskConfig {
    fn to_sql<'b>(&'b self, out: &mut serialize::Output<'b, '_, Sqlite>) -> serialize::Result {
        out.set_value(self.encode_to_vec());
        Ok(serialize::IsNull::No)
    }
}
