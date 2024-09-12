tonic::include_proto!("api/api");

pub const DESCRIPTOR: &[u8] = tonic::include_file_descriptor_set!("api/proto_descriptor");

#[cfg(feature = "diesel")]
use diesel::{backend::Backend, deserialize, serialize, sql_types::Binary};
#[cfg(feature = "diesel")]
use prost::Message;
#[cfg(feature = "diesel")]
use std::io::Cursor;

#[cfg(feature = "diesel")]
impl<ST, DB> deserialize::FromSql<ST, DB> for ChMaskConfig
where
    DB: Backend,
    *const [u8]: deserialize::FromSql<ST, DB>,
{
    fn from_sql(value: DB::RawValue<'_>) -> deserialize::Result<Self> {
        let bytes = <Vec<u8> as deserialize::FromSql<ST, DB>>::from_sql(value)?;
        Ok(ChMaskConfig::decode(&mut Cursor::new(bytes))?)
    }
}

#[cfg(feature = "diesel")]
impl serialize::ToSql<Binary, diesel::pg::Pg> for ChMaskConfig
where
    [u8]: serialize::ToSql<Binary, diesel::pg::Pg>,
{
    fn to_sql(&self, out: &mut serialize::Output<'_, '_, diesel::pg::Pg>) -> serialize::Result {
        <[u8] as serialize::ToSql<Binary, diesel::pg::Pg>>::to_sql(
            &self.encode_to_vec(),
            &mut out.reborrow(),
        )
    }
}
