// Copyright (c) 2022 MASSA LABS <info@massa.net>
/// This file defines testing tools related to the configuration
use massa_models::{Address, Amount};
use std::collections::BTreeMap;
use std::io::Seek;
use tempfile::{NamedTempFile, TempDir};

use crate::LedgerConfig;

/// Default value of `LedgerConfig` used for tests
impl Default for LedgerConfig {
    fn default() -> Self {
        LedgerConfig {
            // unused by the mock (you can use `LedgerConfig::sample()` to get
            // a NamedTempFile in addition)
            initial_sce_ledger_path: "".into(),
            disk_ledger_path: "".into(),
        }
    }
}

impl LedgerConfig {
    /// get ledger and ledger configuration
    pub fn sample(ledger: &BTreeMap<Address, Amount>) -> (Self, NamedTempFile, TempDir) {
        let initial_ledger = NamedTempFile::new().expect("cannot create temp file");
        let disk_ledger = TempDir::new().expect("cannot create temp directory");
        serde_json::to_writer_pretty(initial_ledger.as_file(), &ledger)
            .expect("unable to write ledger file");
        initial_ledger
            .as_file()
            .seek(std::io::SeekFrom::Start(0))
            .expect("could not seek file");
        (
            Self {
                initial_sce_ledger_path: initial_ledger.path().to_path_buf(),
                disk_ledger_path: disk_ledger.path().to_path_buf(),
            },
            initial_ledger,
            disk_ledger,
        )
    }
}
