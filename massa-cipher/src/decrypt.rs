// Copyright (c) 2022 MASSA LABS <info@massa.net>

//! massa-cipher decryption module.
//!
//! Read `lib.rs` module documentation for more information.

use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use pbkdf2::{
    password_hash::{PasswordHasher, SaltString},
    Pbkdf2,
};

use crate::constants::{HASH_PARAMS, NONCE_SIZE, SALT_SIZE};
use crate::error::CipherError;
use massa_serialization::{DeserializeError, Deserializer, U32VarIntDeserializer};

use std::ops::Bound::Included;

/// Decryption function using AES-GCM cipher.
///
/// Read `lib.rs` module documentation for more information.
pub fn decrypt(password: &str, data: &[u8]) -> Result<(u32, Vec<u8>), CipherError> {
    // parse cipher version
    let (rest, version) = U32VarIntDeserializer::new(Included(0), Included(u32::MAX))
        .deserialize::<DeserializeError>(data)
        .map_err(|_| {
            CipherError::DecryptionError(
                "wallet file truncated: version missing or incomplete".to_string(),
            )
        })?;

    // parse PBKDF2 salt
    let salt_data = rest.get(..SALT_SIZE).ok_or_else(|| {
        CipherError::DecryptionError(
            "wallet file truncated: salt missing or incomplete".to_string(),
        )
    })?;
    let salt = SaltString::new(std::str::from_utf8(salt_data)?)
        .map_err(|e| CipherError::DecryptionError(e.to_string()))?;

    // compute PBKDF2 password hash
    let password_hash = Pbkdf2
        .hash_password_customized(password.as_bytes(), None, None, HASH_PARAMS, &salt)
        .map_err(|e| CipherError::DecryptionError(e.to_string()))?
        .hash
        .expect("content is missing after a successful hash");

    // parse AES-GCM nonce
    let nonce_end_index = SALT_SIZE + NONCE_SIZE;
    let nonce = Nonce::from_slice(rest.get(SALT_SIZE..nonce_end_index).ok_or_else(|| {
        CipherError::DecryptionError(
            "wallet file truncated: nonce missing or incomplete".to_string(),
        )
    })?);

    // decrypt the data
    let cipher = Aes256Gcm::new_from_slice(password_hash.as_bytes()).expect("invalid size key");
    let decrypted_bytes = cipher
        .decrypt(
            nonce,
            rest.get(nonce_end_index..).ok_or_else(|| {
                CipherError::DecryptionError(
                    "wallet file truncated: encrypted data missing or incomplete".to_string(),
                )
            })?,
        )
        .map_err(|_| {
            CipherError::DecryptionError("wrong password or corrupted data".to_string())
        })?;
    Ok((version, decrypted_bytes))
}
