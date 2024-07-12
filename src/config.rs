use serde::{Serialize, Deserialize};
use crate::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub host: String,
    pub thread_count: usize,
    #[serde(default)]
    pub compression_algorithm: CompressionAlgConfig,
    #[serde(default)]
    pub protocol: Protocol,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: "localhost:1234".to_string(),                     
            thread_count: 8,
            compression_algorithm: CompressionAlgConfig::default(),
            protocol: Protocol::default(),
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let config = match std::fs::read_to_string(paths::config_file()) {
            Ok(config) => config,
            Err(e) => {  
                match e.kind() {
                    std::io::ErrorKind::NotFound => {     
                        println!("Creating default config file at {}", paths::config_file().to_str().unwrap());                   
                        let default_config = toml::to_string(&Config::default()).expect("Failed to serialize default config");
                        paths::create_dir_if_not_exists(&paths::config_dir())?;
                        std::fs::write(paths::config_file(), &default_config).expect("Failed to write default config");
                        default_config
                    },
                    std::io::ErrorKind::PermissionDenied => {
                        eprintln!("Could not read config file at {}: Permission denied", paths::config_file().to_str().unwrap());
                        eprintln!("Please check the file permissions and try again.");                        
                        std::process::exit(1);
                    },
                    std::io::ErrorKind::IsADirectory => {
                        eprintln!("Could not read config file at {}: Is a directory", paths::config_file().to_str().unwrap());
                        eprintln!("Please check the file permissions and try again.");                        
                        std::process::exit(1);
                    }
                    _ => return Err(Error::Io(e))
                }
            }
        };
        Ok(toml::from_str(&config).expect("Failed to parse config"))
    }
}