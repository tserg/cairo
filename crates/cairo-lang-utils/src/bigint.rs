#[cfg(test)]
#[path = "bigint_test.rs"]
mod test;

use std::ops::Neg;

use num_bigint::{BigInt, BigUint, ToBigInt};
use num_traits::{Num, Signed};
use parity_scale_codec::{Decode, Encode};
use schemars::JsonSchema;
use serde::ser::Serializer;
use serde::{Deserialize, Deserializer, Serialize};

/// A wrapper for BigUint that serializes as hex.
#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BigUintAsHex {
    /// A field element that encodes the signature of the called function.
    #[serde(serialize_with = "serialize_big_uint", deserialize_with = "deserialize_big_uint")]
    pub value: BigUint,
}

fn deserialize_from_str<'a, D>(s: &str) -> Result<BigUint, D::Error>
where
    D: Deserializer<'a>,
{
    match s.strip_prefix("0x") {
        Some(num_no_prefix) => BigUint::from_str_radix(num_no_prefix, 16)
            .map_err(|error| serde::de::Error::custom(format!("{error}"))),
        None => Err(serde::de::Error::custom(format!("{s} does not start with `0x` is missing."))),
    }
}

pub fn serialize_big_uint<S>(num: &BigUint, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&format!("{num:#x}"))
}

pub fn deserialize_big_uint<'a, D>(deserializer: D) -> Result<BigUint, D::Error>
where
    D: Deserializer<'a>,
{
    let s = &String::deserialize(deserializer)?;
    deserialize_from_str::<D>(s)
}

// A wrapper for BigInt that serializes as hex.
#[derive(Default, Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct BigIntAsHex {
    /// A field element that encodes the signature of the called function.
    #[serde(serialize_with = "serialize_big_int", deserialize_with = "deserialize_big_int")]
    #[schemars(schema_with = "big_int_schema")]
    pub value: BigInt,
}

// BigInt doesn't implement JsonSchema, so we need to manually define it.
fn big_int_schema(gen: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
    #[allow(dead_code)]
    #[allow(clippy::enum_variant_names)]
    #[derive(JsonSchema)]
    pub enum Sign {
        Minus,
        NoSign,
        Plus,
    }

    #[allow(dead_code)]
    #[derive(JsonSchema)]
    pub struct BigUint {
        data: Vec<u64>, // BigDigit is u64 or u32.
    }

    #[allow(dead_code)]
    #[derive(JsonSchema)]
    struct BigInt {
        sign: Sign,
        data: BigUint,
    }

    gen.subschema_for::<BigInt>()
}

impl<T: Into<BigInt>> From<T> for BigIntAsHex {
    fn from(x: T) -> Self {
        Self { value: x.into() }
    }
}

pub fn serialize_big_int<S>(num: &BigInt, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&format!(
        "{}{:#x}",
        if num.is_negative() { "-" } else { "" },
        num.magnitude()
    ))
}

pub fn deserialize_big_int<'a, D>(deserializer: D) -> Result<BigInt, D::Error>
where
    D: Deserializer<'a>,
{
    let s = &String::deserialize(deserializer)?;
    match s.strip_prefix('-') {
        Some(abs_value) => Ok(deserialize_from_str::<D>(abs_value)?.to_bigint().unwrap().neg()),
        None => Ok(deserialize_from_str::<D>(s)?.to_bigint().unwrap()),
    }
}

impl Encode for BigIntAsHex {
    fn size_hint(&self) -> usize {
        // sign + len packed in the same byte, it allows numbers of byte size up to 63 (2**504),
        // data.
        let bits = self.value.bits() as usize;
        core::mem::size_of::<u8>() + bits / 8 + if bits % 8 != 0 { 1 } else { 0 }
    }

    /// /!\ Warning this function panics if the number encoded is too big (>= 2**504)
    fn encode_to<T: parity_scale_codec::Output + ?Sized>(&self, dest: &mut T) {
        let (sign, data) = self.value.to_bytes_le();
        assert!(data.len() <= 63, "Can't encode numbers longer than 63 bytes");
        // Pack sign + number byte size.
        ((match sign {
            num_bigint::Sign::Minus => 0u8,
            num_bigint::Sign::NoSign => 1u8,
            num_bigint::Sign::Plus => 2u8,
        } << 6)
            + data.len() as u8)
            .encode_to(dest);
        dest.write(&data);
    }
}

impl Decode for BigIntAsHex {
    fn decode<I: parity_scale_codec::Input>(
        input: &mut I,
    ) -> Result<Self, parity_scale_codec::Error> {
        let sign_and_len = input.read_byte()?;
        let sign = match sign_and_len >> 6 {
            0u8 => num_bigint::Sign::Minus,
            1u8 => num_bigint::Sign::NoSign,
            2u8 => num_bigint::Sign::Plus,
            _ => {
                return Err(parity_scale_codec::Error::from("Bad sign encoding."));
            }
        };
        let len = sign_and_len & 0b00111111;
        let mut buffer = vec![0; len as usize];
        input.read(&mut buffer)?;
        Ok(Self { value: BigInt::from_bytes_le(sign, buffer.as_slice()) })
    }
}
