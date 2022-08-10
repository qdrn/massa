// Copyright (c) 2022 MASSA LABS <info@massa.net>

use crate::error::MassaHashError;
use crate::settings::HASH_SIZE_BYTES;
use massa_serialization::Deserializer;
use nom::{
    error::{context, ContextError, ParseError},
    IResult,
};
use std::{cmp::Ordering, convert::TryInto, str::FromStr};

/// Hash wrapper, the underlying hash type is Blake3
#[derive(Eq, PartialEq, Copy, Clone, Hash)]
pub struct Hash(blake3::Hash);

impl PartialOrd for Hash {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.0.as_bytes().partial_cmp(other.0.as_bytes())
    }
}

impl Ord for Hash {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.as_bytes().cmp(other.0.as_bytes())
    }
}

impl std::fmt::Display for Hash {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.to_bs58_check())
    }
}

impl std::fmt::Debug for Hash {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.to_bs58_check())
    }
}

impl Hash {
    /// Compute a hash from data.
    ///
    /// # Example
    ///  ```
    /// # use massa_hash::Hash;
    /// let hash = Hash::compute_from(&"hello world".as_bytes());
    /// ```
    pub fn compute_from(data: &[u8]) -> Self {
        Hash(blake3::hash(data))
    }

    /// Serialize a Hash using `bs58` encoding with checksum.
    ///
    /// # Example
    ///  ```
    /// # use massa_hash::Hash;
    /// let hash = Hash::compute_from(&"hello world".as_bytes());
    /// let serialized: String = hash.to_bs58_check();
    /// ```
    pub fn to_bs58_check(&self) -> String {
        bs58::encode(self.to_bytes()).with_check().into_string()
    }

    /// Serialize a Hash as bytes.
    ///
    /// # Example
    ///  ```
    /// # use massa_hash::Hash;
    /// let hash = Hash::compute_from(&"hello world".as_bytes());
    /// let serialized = hash.to_bytes();
    /// ```
    pub fn to_bytes(&self) -> &[u8; HASH_SIZE_BYTES] {
        self.0.as_bytes()
    }

    /// Convert into bytes.
    ///
    /// # Example
    ///  ```
    /// # use massa_hash::Hash;
    /// let hash = Hash::compute_from(&"hello world".as_bytes());
    /// let serialized = hash.into_bytes();
    /// ```
    pub fn into_bytes(self) -> [u8; HASH_SIZE_BYTES] {
        *self.0.as_bytes()
    }

    /// Deserialize using `bs58` encoding with checksum.
    ///
    /// # Example
    ///  ```
    /// # use serde::{Deserialize, Serialize};
    /// # use massa_hash::Hash;
    /// let hash = Hash::compute_from(&"hello world".as_bytes());
    /// let serialized: String = hash.to_bs58_check();
    /// let deserialized: Hash = Hash::from_bs58_check(&serialized).unwrap();
    /// ```
    pub fn from_bs58_check(data: &str) -> Result<Hash, MassaHashError> {
        let decoded_bs58_check = bs58::decode(data)
            .with_check(None)
            .into_vec()
            .map_err(|err| MassaHashError::ParsingError(format!("{}", err)))?;
        Ok(Hash::from_bytes(
            &decoded_bs58_check
                .as_slice()
                .try_into()
                .map_err(|err| MassaHashError::ParsingError(format!("{}", err)))?,
        ))
    }

    /// Deserialize a Hash as bytes.
    ///
    /// # Example
    ///  ```
    /// # use serde::{Deserialize, Serialize};
    /// # use massa_hash::Hash;
    /// let hash = Hash::compute_from(&"hello world".as_bytes());
    /// let serialized = hash.into_bytes();
    /// let deserialized: Hash = Hash::from_bytes(&serialized);
    /// ```
    pub fn from_bytes(data: &[u8; HASH_SIZE_BYTES]) -> Hash {
        Hash(blake3::Hash::from(*data))
    }
}

/// Deserializer for `Hash`
#[derive(Default)]
pub struct HashDeserializer;

impl HashDeserializer {
    /// Creates a deserializer for `Hash`
    pub const fn new() -> Self {
        Self
    }
}

impl Deserializer<Hash> for HashDeserializer {
    fn deserialize<'a, E: ParseError<&'a [u8]> + ContextError<&'a [u8]>>(
        &self,
        buffer: &'a [u8],
    ) -> IResult<&'a [u8], Hash, E> {
        context("Failed hash deserialization", |input: &'a [u8]| {
            if buffer.len() < HASH_SIZE_BYTES {
                return Err(nom::Err::Error(ParseError::from_error_kind(
                    input,
                    nom::error::ErrorKind::LengthValue,
                )));
            }
            Ok((
                &buffer[HASH_SIZE_BYTES..],
                Hash::from_bytes(&buffer[..HASH_SIZE_BYTES].try_into().map_err(|_| {
                    nom::Err::Error(ParseError::from_error_kind(
                        input,
                        nom::error::ErrorKind::Fail,
                    ))
                })?),
            ))
        })(buffer)
    }
}

impl ::serde::Serialize for Hash {
    /// `::serde::Serialize` trait for Hash
    /// if the serializer is human readable,
    /// serialization is done using `serialize_bs58_check`
    /// else, it uses `serialize_binary`
    ///
    /// # Example
    ///
    /// Human readable serialization :
    /// ```
    /// # use serde::{Deserialize, Serialize};
    /// # use massa_hash::Hash;
    /// let hash = Hash::compute_from(&"hello world".as_bytes());
    /// let serialized: String = serde_json::to_string(&hash).unwrap();
    /// ```
    ///
    fn serialize<S: ::serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        if s.is_human_readable() {
            s.collect_str(&self.to_bs58_check())
        } else {
            s.serialize_bytes(self.to_bytes())
        }
    }
}

impl<'de> ::serde::Deserialize<'de> for Hash {
    /// `::serde::Deserialize` trait for Hash
    /// if the deserializer is human readable,
    /// deserialization is done using `deserialize_bs58_check`
    /// else, it uses `deserialize_binary`
    ///
    /// # Example
    ///
    /// Human readable deserialization :
    /// ```
    /// # use massa_hash::Hash;
    /// # use serde::{Deserialize, Serialize};
    /// let hash = Hash::compute_from(&"hello world".as_bytes());
    /// let serialized: String = serde_json::to_string(&hash).unwrap();
    /// let deserialized: Hash = serde_json::from_str(&serialized).unwrap();
    /// ```
    ///
    fn deserialize<D: ::serde::Deserializer<'de>>(d: D) -> Result<Hash, D::Error> {
        if d.is_human_readable() {
            struct Base58CheckVisitor;

            impl<'de> ::serde::de::Visitor<'de> for Base58CheckVisitor {
                type Value = Hash;

                fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                    formatter.write_str("an ASCII base58check string")
                }

                fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
                where
                    E: ::serde::de::Error,
                {
                    if let Ok(v_str) = std::str::from_utf8(v) {
                        Hash::from_bs58_check(v_str).map_err(E::custom)
                    } else {
                        Err(E::invalid_value(::serde::de::Unexpected::Bytes(v), &self))
                    }
                }

                fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
                where
                    E: ::serde::de::Error,
                {
                    Hash::from_bs58_check(v).map_err(E::custom)
                }
            }
            d.deserialize_str(Base58CheckVisitor)
        } else {
            struct BytesVisitor;

            impl<'de> ::serde::de::Visitor<'de> for BytesVisitor {
                type Value = Hash;

                fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                    formatter.write_str("a bytestring")
                }

                fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
                where
                    E: ::serde::de::Error,
                {
                    Ok(Hash::from_bytes(v.try_into().map_err(E::custom)?))
                }
            }

            d.deserialize_bytes(BytesVisitor)
        }
    }
}

impl FromStr for Hash {
    type Err = MassaHashError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Hash::from_bs58_check(s)
    }
}

#[cfg(test)]
mod tests {
    use serial_test::serial;

    use super::*;

    fn example() -> Hash {
        Hash::compute_from("hello world".as_bytes())
    }

    #[test]
    #[serial]
    fn test_serde_json() {
        let hash = example();
        let serialized = serde_json::to_string(&hash).unwrap();
        let deserialized = serde_json::from_str(&serialized).unwrap();
        assert_eq!(hash, deserialized)
    }

    #[test]
    #[serial]
    fn test_hash() {
        let data = "abc".as_bytes();
        let hash = Hash::compute_from(data);
        let hash_ref: [u8; HASH_SIZE_BYTES] = [
            100, 55, 179, 172, 56, 70, 81, 51, 255, 182, 59, 117, 39, 58, 141, 181, 72, 197, 88,
            70, 93, 121, 219, 3, 253, 53, 156, 108, 213, 189, 157, 133,
        ];
        assert_eq!(hash.into_bytes(), hash_ref);
    }
}
