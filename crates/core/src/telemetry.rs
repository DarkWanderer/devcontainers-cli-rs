use anyhow::anyhow;
use std::error::Error as StdError;

use tracing_subscriber::EnvFilter;

use crate::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    Auto,
    Text,
    Json,
}

impl Default for LogFormat {
    fn default() -> Self {
        LogFormat::Auto
    }
}

pub fn init(level: &str, format: LogFormat) -> Result<()> {
    let env_filter = EnvFilter::try_new(level).unwrap_or_else(|_| EnvFilter::new("info"));

    let fmt = tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(false)
        .with_level(true);

    match format {
        LogFormat::Json => fmt
            .json()
            .try_init()
            .map_err(|err: Box<dyn StdError + Send + Sync>| anyhow!(err))?,
        LogFormat::Auto | LogFormat::Text => fmt
            .try_init()
            .map_err(|err: Box<dyn StdError + Send + Sync>| anyhow!(err))?,
    }

    Ok(())
}
