// Copyright (c) 2022 MASSA LABS <info@massa.net>

#![doc = include_str!("../endorsements.md")]
#![feature(async_closure)]
#![feature(hash_drain_filter)]
#![feature(map_first_last)]
#![feature(int_roundings)]
#![warn(missing_docs)]
#![warn(unused_crate_dependencies)]
#[macro_use]
extern crate massa_logging;

mod consensus_worker;

// Tools as starting controller etc...
mod tools;
pub use tools::start_consensus_controller;

#[cfg(test)]
mod tests;
