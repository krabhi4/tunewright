use crate::config::Config;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct AppState {
    pub config: Config,
    pub data_root: PathBuf,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        let data_root = config.data_dir.clone();
        Self { config, data_root }
    }
}
