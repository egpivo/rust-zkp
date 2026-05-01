pub mod biguint_string {
    use num_bigint::BigUint;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &BigUint, ser: S) -> Result<S::Ok, S::Error> {
        ser.serialize_str(&v.to_string())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(de: D) -> Result<BigUint, D::Error> {
        let s = String::deserialize(de)?;
        BigUint::parse_bytes(s.as_bytes(), 10)
            .ok_or_else(|| serde::de::Error::custom("invalid BigUint string"))
    }
}
