// Copyright (c) 2022 MASSA LABS <info@massa.net>
//! Signature management

#![warn(missing_docs)]
#![warn(unused_crate_dependencies)]
mod error;
mod signature_impl;

pub use error::MassaSignatureError;
pub use signature_impl::{
    verify_signature_batch, KeyPair, PublicKey, PublicKeyDeserializer, Signature,
    SignatureDeserializer, PUBLIC_KEY_SIZE_BYTES, SECRET_KEY_BYTES_SIZE, SIGNATURE_SIZE_BYTES,
};
