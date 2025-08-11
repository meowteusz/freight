use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use anyhow::{Result, Context};
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub source_path: PathBuf,
    pub dest_path: PathBuf,
    pub thresholds: Thresholds,
    pub rsync_flags: String,
    pub retry_attempts: u32,
    pub socket_retry_interval: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Thresholds {
    pub large_directory_size: String,
    pub parallel_workers: u32,
}

impl Config {
    pub fn load_or_create(source: &str, dest: &str) -> Result<Self> {
        let source_path = PathBuf::from(source);
        let config_path = source_path.join(".freight/config.json");
        
        if config_path.exists() {
            Self::load(&config_path)
        } else {
            let config = Self::default_with_paths(source, dest);
            config.save(&config_path)?;
            Ok(config)
        }
    }
    
    pub fn load(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read config from {}", path.display()))?;
        
        serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse config from {}", path.display()))
    }
    
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create config directory {}", parent.display()))?;
        }
        
        let content = serde_json::to_string_pretty(self)
            .context("Failed to serialize config")?;
        
        fs::write(path, content)
            .with_context(|| format!("Failed to write config to {}", path.display()))
    }
    
    fn default_with_paths(source: &str, dest: &str) -> Self {
        Self {
            source_path: PathBuf::from(source),
            dest_path: PathBuf::from(dest),
            thresholds: Thresholds {
                large_directory_size: "3GB".to_string(),
                parallel_workers: 5,
            },
            rsync_flags: "-avxHAX --numeric-ids --compress".to_string(),
            retry_attempts: 3,
            socket_retry_interval: 10,
        }
    }
    
    pub fn freight_dir(&self) -> PathBuf {
        self.source_path.join(".freight")
    }
    
    pub fn init_project(source_path: &str) -> Result<()> {
        let source = PathBuf::from(source_path);
        let freight_dir = source.join(".freight");
        
        // Create .freight directory
        fs::create_dir_all(&freight_dir)
            .with_context(|| format!("Failed to create .freight directory at {}", freight_dir.display()))?;
        
        // Create .freight-root marker file
        let freight_root = freight_dir.join(".freight-root");
        fs::write(&freight_root, "")
            .with_context(|| format!("Failed to create .freight-root marker at {}", freight_root.display()))?;
        
        // Always create config.json with placeholder destination
        let config = Self::default_with_paths(source_path, "/path/to/destination");
        let config_path = freight_dir.join("config.json");
        config.save(&config_path)?;
        
        Ok(())
    }
}