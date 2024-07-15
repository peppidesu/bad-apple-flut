use std::collections::HashMap;

use crate::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {    
    #[serde(default)]
    pub args: Args,
    #[serde(default)]
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub targets: HashMap<String, Target>
}

#[derive(Hash, Clone, Debug, Serialize, Deserialize)]
pub struct Target {
    pub host: String,
    pub protocol: Protocol,
    #[serde(default)]
    pub canvas: u8,
}

impl Config {
    pub fn load() -> Result<Self> {
        let config = match std::fs::read_to_string(paths::config_file()) {
            Ok(config) => config,
            Err(e) => match e.kind() {
                std::io::ErrorKind::NotFound => {
                    println!(
                        "Creating default config file at {}",
                        paths::config_file().to_str().unwrap()
                    );
                    
                    paths::create_dir_if_not_exists(&paths::config_dir());
                    
                    let default_config = toml::to_string(&Config::default())
                        .expect("Failed to serialize default config");

                    std::fs::write(paths::config_file(), &default_config)
                        .expect("Failed to write default config");
                    
                    default_config
                }
                std::io::ErrorKind::PermissionDenied => {
                    eprintln!(
                        "Could not read config file at {}: Permission denied",
                        paths::config_file().to_str().unwrap()
                    );
                    eprintln!("Please check the file permissions and try again.");
                    
                    std::process::exit(1);
                }
                _ => return Err(Error::Io(e)),
                
            },
        };
        Ok(toml::from_str(&config).map_err(|e| Error::FileParseError(e.to_string()))?)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            args: Args::config_default(),
            targets: HashMap::new(),
        }
    }
}