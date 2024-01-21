use serde::{Serialize, Deserialize};


#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub host: String,
    pub cache_path: String,
    pub thread_count: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: "localhost:1234".to_string(),
            cache_path: "./cache".to_string(),            
            thread_count: 8,
        }
    }
}

impl Config {
    pub fn load() -> Self {
        let config = match std::fs::read_to_string("config.toml") {
            Ok(config) => config,
            Err(_) => {
                let config = toml::to_string(&Self::default()).unwrap();
                std::fs::write("config.toml", &config).unwrap();
                config
            }
        };
        toml::from_str(&config).unwrap()
    }
}