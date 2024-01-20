use std::hash::{Hasher, Hash};
use std::fs::File;
use std::io::{BufReader, BufRead};

use crate::CACHE_ID_PATH;

pub struct CacheKey {
    input: String,
    width: i32,
    height: i32,
}
impl CacheKey {
    pub fn new(input: String, width: i32, height: i32) -> Self {
        Self {
            input,
            width,
            height,
        }
    }
}

pub fn gen_cache_id(key: &CacheKey) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    key.input.hash(&mut hasher);
    key.width.hash(&mut hasher);
    key.height.hash(&mut hasher);
    hasher.finish()
}

pub fn is_cache_valid(key: &CacheKey) -> std::io::Result<bool> {
    let hash = gen_cache_id(key);

    let cache_id = File::open(CACHE_ID_PATH)?;
    let mut reader = BufReader::new(cache_id);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let old_hash = line.parse::<u64>()
        .map_err(|_| std::io::Error::new(
            std::io::ErrorKind::InvalidData, "invalid cache id"
        ))?;

    Ok(hash == old_hash)
}