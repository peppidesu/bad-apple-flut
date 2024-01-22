use std::hash::{Hasher, Hash};
use std::fs::File;
use std::io::{BufReader, BufRead};

use serde::{Serialize, Deserialize};

use crate::{paths, Result, Error};

#[derive(Debug, Hash)]
pub struct CacheKey {
    input: String,
    width: i32,
    height: i32,
    fps: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VideoMetadata {
    pub fps: f64,
    pub frame_count: usize,
}
impl VideoMetadata {
    pub fn load() -> Result<Self> {
        let raw = std::fs::read_to_string(
            paths::cache_metadata()
        )?;

        toml::from_str(&raw)
            .map_err(|e| Error::FileParseError(e.to_string()))
    }
    pub fn create(fps: f64, frame_count: usize) -> Self {
        Self { fps, frame_count }
    }
    pub fn write(&self) -> Result<()> {
        let raw = toml::to_string(self)
            .map_err(|e| Error::FileParseError(e.to_string()))?;

        std::fs::write(
            paths::cache_metadata(),
            raw
        )?;

        Ok(())
    }
}

impl CacheKey {
    pub fn new(input: String, width: i32, height: i32, fps: f64) -> Self {
        Self {
            input,
            width,
            height,
            fps: (fps * (10.0_f64.powi(6))).round() as u64,
        }
    }
}


pub fn gen_cache_id(key: &CacheKey) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    key.hash(&mut hasher);
    hasher.finish()
}

pub fn write_cache_id(key: &CacheKey) -> Result<()> {
    std::fs::write(
        paths::cache_id(),
        gen_cache_id(key).to_string()
    )?;

    Ok(())
}

pub fn is_cache_valid(key: &CacheKey) -> std::io::Result<bool> {
    if !std::path::Path::new(&paths::cache_id()).exists() {
        return Ok(false);
    }

    let hash = gen_cache_id(key);

    let cache_id = File::open(paths::cache_id())?;
    let mut reader = BufReader::new(cache_id);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let old_hash = line.parse::<u64>()
        .map_err(|_| std::io::Error::new(
            std::io::ErrorKind::InvalidData, "invalid cache id"
        ))?;

    Ok(hash == old_hash)
}

pub fn clean_cache() -> Result<()> {
    if paths::cache().exists() {
        std::fs::remove_dir_all(paths::cache())?;
    }
    Ok(())
}