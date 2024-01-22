use serde::{Serialize, Deserialize};

use crate::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub host: String,
    pub thread_count: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: "localhost:1234".to_string(),                     
            thread_count: 8,
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let config = match std::fs::read_to_string(paths::config_file()) {
            Ok(config) => config,
            Err(_) => {
                let config = toml::to_string(&Self::default()).unwrap();
                paths::create_dir_if_not_exists(&paths::config_dir())?;
                std::fs::write(paths::config_file(), &config)?;
                config
            }
        };
        Ok(toml::from_str(&config).unwrap_or(Self::default()))
    }
}