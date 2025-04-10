use base64::prelude::*;
use diesel::backend::Backend;
use diesel::deserialize::{self, FromSql};
use diesel::pg::Pg;
use diesel::serialize::{self, IsNull, Output, ToSql};
use diesel::sql_types::Text;
use sha2::{Digest, Sha256};
use std::io::Write;

#[derive(SqlType)]
#[diesel(postgres_type(name = "encodedhashtype"))]
pub struct EncodedHashType;

#[derive(Debug, FromSqlRow, AsExpression, PartialEq, Eq, PartialOrd, Ord, Clone)]
#[diesel(sql_type = EncodedHashType)]
pub struct EncodedHash {
    inner: String,
}

impl EncodedHash {
    /// ## Safety 
    /// Programmer must ensure the input `st` is properly encoded in Base64 Format. Calling
    /// the safe `hash` function will guarantee this - even if the input was already hashed,
    /// as the data will just be double-hashed (but still valid)
    pub unsafe fn from_raw(st: String) -> EncodedHash {
        EncodedHash { inner: st }
    }
    pub fn hash<T: AsRef<[u8]>>(data: T) -> EncodedHash {
        unsafe { EncodedHash::from_raw(BASE64_STANDARD.encode(Sha256::digest(data))) }
    }
}

impl ToSql<EncodedHashType, Pg> for EncodedHash {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        out.write_all(self.inner.as_bytes())?;
        Ok(IsNull::No)
    }
}

impl FromSql<EncodedHashType, Pg> for EncodedHash {
    fn from_sql(bytes: <Pg as Backend>::RawValue<'_>) -> deserialize::Result<Self> {
        Ok(String::from_utf8(bytes.as_bytes().to_vec())
            .map(|x| unsafe { EncodedHash::from_raw(x) })?)
    }
}

impl PartialEq<String> for EncodedHash {
    fn eq(&self, other: &String) -> bool {
        // We need to hash the string and then compare it.
        self.inner == EncodedHash::hash(other).inner
    }
}