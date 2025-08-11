use crate::{Config, WorkerMessage};
use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;
use tokio::sync::broadcast;
use tracing::{info, error};

#[derive(Debug, Clone, PartialEq)]
pub enum WorkerStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone)]
pub struct WorkerInfo {
    pub tool: String,
    pub directory: PathBuf,
    pub status: WorkerStatus,
    pub pid: Option<u32>,
}

pub struct WorkerManager {
    config: Option<Config>,
    workers: HashMap<String, WorkerInfo>,
}

impl WorkerManager {
    pub fn new() -> Self {
        Self {
            config: None,
            workers: HashMap::new(),
        }
    }
    
    pub fn set_config(&mut self, config: Config) {
        self.config = Some(config);
    }
    
    pub async fn start(&self, mut message_rx: broadcast::Receiver<WorkerMessage>) {
        info!("Worker manager started");
        
        while let Ok(message) = message_rx.recv().await {
            info!("Received worker message: {:?}", message);
            // Handle worker lifecycle events
        }
    }
    
    pub async fn start_migration(&mut self, mut message_rx: broadcast::Receiver<WorkerMessage>) {
        info!("Starting migration workflow");
        
        let config = match &self.config {
            Some(config) => config.clone(),
            None => {
                error!("No configuration provided for migration");
                return;
            }
        };
        
        // Discover directories to migrate
        let directories = match self.discover_directories(&config.source_path).await {
            Ok(dirs) => dirs,
            Err(e) => {
                error!("Failed to discover directories: {}", e);
                return;
            }
        };
        
        info!("Found {} directories to migrate", directories.len());
        
        // Start scanning phase
        for dir in &directories {
            if let Err(e) = self.start_scan_worker(dir).await {
                error!("Failed to start scan worker for {}: {}", dir.display(), e);
            }
        }
        
        // Listen for worker messages and coordinate migration phases
        while let Ok(message) = message_rx.recv().await {
            self.handle_worker_message(message).await;
        }
    }
    
    async fn discover_directories(&self, source_path: &PathBuf) -> Result<Vec<PathBuf>> {
        let mut directories = Vec::new();
        
        let mut entries = tokio::fs::read_dir(source_path).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_dir() && !path.file_name().unwrap().to_str().unwrap().starts_with('.') {
                directories.push(path);
            }
        }
        
        Ok(directories)
    }
    
    async fn start_scan_worker(&mut self, directory: &PathBuf) -> Result<()> {
        info!("Starting scan worker for {}", directory.display());
        
        let mut cmd = Command::new("freight-scan");
        cmd.arg(directory)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        
        let child = cmd.spawn()?;
        let pid = child.id();
        
        let worker_info = WorkerInfo {
            tool: "scan".to_string(),
            directory: directory.clone(),
            status: WorkerStatus::Running,
            pid,
        };
        
        let worker_id = format!("scan:{}", directory.display());
        self.workers.insert(worker_id, worker_info);
        
        // Spawn task to wait for completion
        let directory_clone = directory.clone();
        tokio::spawn(async move {
            match child.wait_with_output().await {
                Ok(output) => {
                    if output.status.success() {
                        info!("Scan completed for {}", directory_clone.display());
                    } else {
                        error!("Scan failed for {}: {}", 
                            directory_clone.display(), 
                            String::from_utf8_lossy(&output.stderr)
                        );
                    }
                }
                Err(e) => {
                    error!("Failed to wait for scan worker: {}", e);
                }
            }
        });
        
        Ok(())
    }
    
    async fn start_migrate_worker(&mut self, directory: &PathBuf) -> Result<()> {
        info!("Starting migrate worker for {}", directory.display());
        
        let config = self.config.as_ref().unwrap();
        let dest_dir = config.dest_path.join(directory.file_name().unwrap());
        
        let mut cmd = Command::new("freight-migrate");
        cmd.arg(directory)
            .arg(&dest_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        
        let child = cmd.spawn()?;
        let pid = child.id();
        
        let worker_info = WorkerInfo {
            tool: "migrate".to_string(),
            directory: directory.clone(),
            status: WorkerStatus::Running,
            pid,
        };
        
        let worker_id = format!("migrate:{}", directory.display());
        self.workers.insert(worker_id, worker_info);
        
        // Spawn task to wait for completion
        let directory_clone = directory.clone();
        tokio::spawn(async move {
            match child.wait_with_output().await {
                Ok(output) => {
                    if output.status.success() {
                        info!("Migration completed for {}", directory_clone.display());
                    } else {
                        error!("Migration failed for {}: {}", 
                            directory_clone.display(), 
                            String::from_utf8_lossy(&output.stderr)
                        );
                    }
                }
                Err(e) => {
                    error!("Failed to wait for migrate worker: {}", e);
                }
            }
        });
        
        Ok(())
    }
    
    async fn handle_worker_message(&mut self, message: WorkerMessage) {
        match message.message_type {
            crate::socket::MessageType::Stop => {
                let worker_id = format!("{}:{}", 
                    message.tool, 
                    message.directory.as_deref().unwrap_or("unknown")
                );
                
                if let Some(worker) = self.workers.get_mut(&worker_id) {
                    worker.status = if message.status.as_deref() == Some("ok") {
                        WorkerStatus::Completed
                    } else {
                        WorkerStatus::Failed
                    };
                    
                    // Check if we should start next phase
                    if message.tool == "scan" && worker.status == WorkerStatus::Completed {
                        if let Some(directory) = &message.directory {
                            let dir_path = PathBuf::from(directory);
                            if let Err(e) = self.start_migrate_worker(&dir_path).await {
                                error!("Failed to start migration for {}: {}", directory, e);
                            }
                        }
                    }
                }
            }
            _ => {
                // Handle other message types as needed
            }
        }
    }
}