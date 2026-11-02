use serde::{Deserialize, Deserializer, Serializer, de::Error};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

pub fn serialize<S: Serializer>(t: &OffsetDateTime, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&t.format(&Rfc3339).map_err(serde::ser::Error::custom)?)
}

pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<OffsetDateTime, D::Error> {
    let s = String::deserialize(d)?;
    OffsetDateTime::parse(&s, &Rfc3339).map_err(D::Error::custom)
}

pub mod option {
    use super::*;

    pub fn serialize<S: Serializer>(t: &Option<OffsetDateTime>, s: S) -> Result<S::Ok, S::Error> {
        match t {
            Some(t) => super::serialize(t, s),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        d: D,
    ) -> Result<Option<OffsetDateTime>, D::Error> {
        match Option::<String>::deserialize(d)? {
            Some(s) => OffsetDateTime::parse(&s, &Rfc3339)
                .map(Some)
                .map_err(D::Error::custom),
            None => Ok(None),
        }
    }
}
