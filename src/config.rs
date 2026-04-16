use std::{path::PathBuf, time::Duration};

use clap::{Parser, ValueEnum};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum Mode {
    Snapshot,
    Secondary,
}

#[derive(Debug, Parser)]
#[command(
    name = "rocksdb-mcp",
    version,
    about = "Streamable-HTTP MCP server for a read-only view of a RocksDB database",
    long_about = None,
)]
pub struct Config {
    #[arg(long, env = "ROCKSDB_PATH", value_name = "PATH")]
    pub db_path: PathBuf,

    #[arg(long, env = "ROCKSDB_MODE", value_enum, default_value_t = Mode::Snapshot)]
    pub mode: Mode,

    #[arg(long, env = "ROCKSDB_SECONDARY_PATH", value_name = "PATH")]
    pub secondary_path: Option<PathBuf>,

    #[arg(
        long,
        env = "ROCKSDB_REFRESH_INTERVAL",
        default_value = "5s",
        value_parser = parse_duration,
        value_name = "DURATION"
    )]
    pub refresh_interval: Duration,

    #[arg(long, env = "MCP_HOST", default_value = "127.0.0.1")]
    pub host: String,

    #[arg(long, env = "MCP_PORT", default_value_t = 8080)]
    pub port: u16,

    #[arg(long, env = "MCP_API_TOKEN", value_name = "TOKEN")]
    pub api_token: Option<String>,
}

impl Config {
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.mode == Mode::Secondary && self.secondary_path.is_none() {
            anyhow::bail!(
                "--secondary-path (ROCKSDB_SECONDARY_PATH) is required when --mode=secondary"
            );
        }
        Ok(())
    }

    pub fn bind_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

fn parse_duration(s: &str) -> Result<Duration, String> {
    humantime::parse_duration(s).map_err(|e| format!("invalid duration '{s}': {e}"))
}
