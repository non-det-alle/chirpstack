use std::ops::{Deref, DerefMut};

use diesel::backend::Backend;
#[cfg(feature = "postgres")]
use diesel::pg::Pg;
use diesel::sql_types::Binary;
#[cfg(feature = "sqlite")]
use diesel::sqlite::Sqlite;
use diesel::{deserialize, serialize};

#[derive(Debug, Clone, PartialEq, AsExpression, FromSqlRow)]
#[diesel(sql_type = diesel::sql_types::Binary)]
pub struct ChMaskConfig(Vec<u32>);

impl ChMaskConfig {
    pub fn new(m: Vec<u32>) -> Self {
        ChMaskConfig(m)
    }
}

impl std::convert::From<Vec<u32>> for ChMaskConfig {
    fn from(u: Vec<u32>) -> Self {
        Self(u)
    }
}

impl std::convert::From<&Vec<u32>> for ChMaskConfig {
    fn from(u: &Vec<u32>) -> Self {
        Self::from(u.clone())
    }
}

impl std::convert::From<ChMaskConfig> for Vec<u32> {
    fn from(val: ChMaskConfig) -> Self {
        val.0
    }
}

impl Deref for ChMaskConfig {
    type Target = Vec<u32>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ChMaskConfig {
    fn deref_mut(&mut self) -> &mut Vec<u32> {
        &mut self.0
    }
}

impl PartialEq<Vec<u32>> for ChMaskConfig {
    fn eq(&self, other: &Vec<u32>) -> bool {
        self.0 == *other
    }
}

impl PartialEq<ChMaskConfig> for Vec<u32> {
    fn eq(&self, other: &ChMaskConfig) -> bool {
        *self == other.0
    }
}

impl<DB> deserialize::FromSql<Binary, DB> for ChMaskConfig
where
    DB: Backend,
    *const [u8]: deserialize::FromSql<Binary, DB>,
{
    fn from_sql(value: DB::RawValue<'_>) -> deserialize::Result<Self> {
        let bindata = <*const [u8] as deserialize::FromSql<Binary, DB>>::from_sql(value)?;
        let cm = (unsafe { &*bindata })
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
            .collect();
        Ok(ChMaskConfig(cm))
    }
}

#[cfg(feature = "postgres")]
impl serialize::ToSql<Binary, Pg> for ChMaskConfig {
    fn to_sql<'b>(&'b self, out: &mut serialize::Output<'b, '_, Pg>) -> serialize::Result {
        let encoded = &self
            .0
            .iter()
            .cloned()
            .flat_map(|c| c.to_le_bytes())
            .collect();
        <Vec<u8> as serialize::ToSql<Binary, Pg>>::to_sql(encoded, &mut out.reborrow())
    }
}

#[cfg(feature = "sqlite")]
impl serialize::ToSql<Binary, Sqlite> for ChMaskConfig {
    fn to_sql<'b>(&'b self, out: &mut serialize::Output<'b, '_, Sqlite>) -> serialize::Result {
        let encoded: Vec<u8> = self
            .0
            .iter()
            .cloned()
            .flat_map(|c| c.to_le_bytes())
            .collect();
        out.set_value(encoded);
        Ok(serialize::IsNull::No)
    }
}
