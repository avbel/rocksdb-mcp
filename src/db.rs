use std::{path::Path, sync::Arc};

use rocksdb::{ColumnFamilyDescriptor, DB, Options};

use crate::config::{Config, Mode};

pub struct Database {
    db: Arc<DB>,
    column_families: Vec<String>,
    mode: Mode,
}

impl Database {
    pub fn open(cfg: &Config) -> anyhow::Result<Self> {
        let cf_names = list_cf_or_default(&cfg.db_path)?;
        let db = match cfg.mode {
            Mode::Snapshot => open_snapshot(&cfg.db_path, &cf_names)?,
            Mode::Secondary => {
                let secondary = cfg
                    .secondary_path
                    .as_deref()
                    .expect("secondary_path required in secondary mode; validated in Config");
                open_secondary(&cfg.db_path, secondary, &cf_names)?
            }
        };
        Ok(Self {
            db: Arc::new(db),
            column_families: cf_names,
            mode: cfg.mode,
        })
    }

    pub fn handle(&self) -> Arc<DB> {
        Arc::clone(&self.db)
    }

    pub fn column_families(&self) -> &[String] {
        &self.column_families
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn get(&self, cf_name: &str, key: &[u8]) -> Result<Option<Vec<u8>>, GetError> {
        let cf = self
            .db
            .cf_handle(cf_name)
            .ok_or_else(|| GetError::UnknownColumnFamily {
                requested: cf_name.to_string(),
                available: self.column_families.clone(),
            })?;
        self.db.get_cf(&cf, key).map_err(GetError::RocksDb)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum GetError {
    #[error(
        "unknown column family '{requested}'; call list_column_families to see available CFs (currently: {available:?})"
    )]
    UnknownColumnFamily {
        requested: String,
        available: Vec<String>,
    },
    #[error("rocksdb error: {0}")]
    RocksDb(#[from] rocksdb::Error),
}

fn list_cf_or_default(path: &Path) -> anyhow::Result<Vec<String>> {
    match DB::list_cf(&Options::default(), path) {
        Ok(names) => Ok(names),
        Err(e) => {
            // An empty/new DB may not have any CFs yet; fall back to "default".
            tracing::warn!(error = %e, "DB::list_cf failed; assuming single 'default' CF");
            Ok(vec!["default".to_string()])
        }
    }
}

fn cf_descriptors(names: &[String]) -> Vec<ColumnFamilyDescriptor> {
    names
        .iter()
        .map(|n| ColumnFamilyDescriptor::new(n, Options::default()))
        .collect()
}

fn open_snapshot(path: &Path, cf_names: &[String]) -> anyhow::Result<DB> {
    let mut opts = Options::default();
    opts.create_if_missing(false);
    opts.create_missing_column_families(false);
    let cfs = cf_descriptors(cf_names);
    let db = DB::open_cf_descriptors_read_only(&opts, path, cfs, false)?;
    Ok(db)
}

fn open_secondary(primary: &Path, secondary: &Path, cf_names: &[String]) -> anyhow::Result<DB> {
    let mut opts = Options::default();
    opts.create_if_missing(false);
    opts.create_missing_column_families(false);
    // Required by the RocksDB secondary protocol: keep all FDs open so files
    // remain accessible after the primary unlinks them.
    opts.set_max_open_files(-1);
    let cfs = cf_descriptors(cf_names);
    let db = DB::open_cf_descriptors_as_secondary(&opts, primary, secondary, cfs)?;
    Ok(db)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use rocksdb::{DB, Options};
    use tempfile::TempDir;

    use super::*;

    fn seed_primary(path: &PathBuf, cfs: &[&str], rows: &[(&str, &str, &str)]) {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);
        let cf_names: Vec<String> = cfs.iter().map(|s| s.to_string()).collect();
        let db = DB::open_cf(&opts, path, &cf_names).unwrap();
        for (cf_name, k, v) in rows {
            let cf = db.cf_handle(cf_name).unwrap();
            db.put_cf(&cf, k.as_bytes(), v.as_bytes()).unwrap();
        }
    }

    #[test]
    fn snapshot_lists_and_reads() {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("db");
        seed_primary(
            &db_path,
            &["default", "meta"],
            &[("default", "a", "1"), ("meta", "version", "42")],
        );

        let cfg = Config {
            db_path: db_path.clone(),
            mode: Mode::Snapshot,
            secondary_path: None,
            refresh_interval: std::time::Duration::from_secs(5),
            host: "127.0.0.1".into(),
            port: 0,
            api_token: None,
        };
        let db = Database::open(&cfg).unwrap();

        let mut cfs: Vec<String> = db.column_families().to_vec();
        cfs.sort();
        assert_eq!(cfs, vec!["default".to_string(), "meta".to_string()]);

        assert_eq!(db.get("default", b"a").unwrap().as_deref(), Some(&b"1"[..]));
        assert_eq!(
            db.get("meta", b"version").unwrap().as_deref(),
            Some(&b"42"[..]),
        );
        assert!(db.get("meta", b"missing").unwrap().is_none());

        let err = db.get("nope", b"a").unwrap_err();
        assert!(matches!(err, GetError::UnknownColumnFamily { .. }));
    }
}
