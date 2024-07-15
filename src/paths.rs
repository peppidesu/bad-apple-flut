use std::path::PathBuf;

pub fn cache() -> PathBuf {
    dirs::cache_dir().unwrap()
        .join("bad-apple-flut")
}
pub fn cache_id() -> PathBuf {
    cache().join("cache_id")
}
pub fn cache_frames() -> PathBuf {
    cache().join("frames")
}
pub fn cache_metadata() -> PathBuf {
    cache().join(format!("metadata"))
}
pub fn frame_file(idx: usize) -> PathBuf {
    cache_frames().join(format!("frame{}.ppm", idx))
}

pub fn config_dir() -> PathBuf {
    dirs::config_dir().unwrap()
        .join("bad-apple-flut")
}
pub fn config_file() -> PathBuf {
    config_dir().join("config.toml")
}

pub fn create_dir_if_not_exists(path: &PathBuf) {
    if !path.exists() {
        std::fs::create_dir_all(path).expect("Failed to create config directory");
    }    
}
