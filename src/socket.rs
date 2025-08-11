use tokio::net::{UnixListener, UnixStream};
use tokio::io::{AsyncBufReadExt, BufReader};
use serde::{Deserialize, Serialize};
use anyhow::{Result, Context};
use tracing::{info, warn, error, debug};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};

pub const SOCKET_PATH: &str = "/tmp/freight-daemon.sock";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerMessage {
    pub message_type: MessageType,
    pub tool: String,
    pub directory: Option<String>,
    pub status: Option<String>,
    pub bytes: Option<u64>,
    pub message: Option<String>,
    pub host: Option<String>,
    pub pid: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageType {
    Hello,
    Start,
    Progress,
    Stop,
}

#[derive(Debug, Clone)]
pub struct WorkerState {
    pub tool: String,
    pub directory: Option<String>,
    pub status: String,
    pub last_message: Option<String>,
    pub bytes_transferred: Option<u64>,
    pub host: Option<String>,
    pub pid: Option<u32>,
    pub connected: bool,
}

pub struct SocketServer {
    workers: Arc<RwLock<HashMap<String, WorkerState>>>,
    message_tx: broadcast::Sender<WorkerMessage>,
}

impl SocketServer {
    pub fn new() -> (Self, broadcast::Receiver<WorkerMessage>) {
        let (message_tx, message_rx) = broadcast::channel(1000);
        
        (
            Self {
                workers: Arc::new(RwLock::new(HashMap::new())),
                message_tx,
            },
            message_rx,
        )
    }
    
    pub async fn start(&self) -> Result<()> {
        // Remove existing socket file
        let _ = std::fs::remove_file(SOCKET_PATH);
        
        let listener = UnixListener::bind(SOCKET_PATH)
            .context("Failed to bind Unix socket")?;
        
        info!("Socket server listening on {}", SOCKET_PATH);
        
        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let workers = Arc::clone(&self.workers);
                    let message_tx = self.message_tx.clone();
                    
                    tokio::spawn(async move {
                        if let Err(e) = handle_worker_connection(stream, workers, message_tx).await {
                            error!("Worker connection error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    error!("Failed to accept connection: {}", e);
                }
            }
        }
    }
    
    pub async fn get_workers(&self) -> HashMap<String, WorkerState> {
        self.workers.read().await.clone()
    }
}

async fn handle_worker_connection(
    stream: UnixStream,
    workers: Arc<RwLock<HashMap<String, WorkerState>>>,
    message_tx: broadcast::Sender<WorkerMessage>,
) -> Result<()> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    let mut worker_id: Option<String> = None;
    
    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) => {
                // Connection closed
                if let Some(id) = &worker_id {
                    let mut workers_guard = workers.write().await;
                    if let Some(worker) = workers_guard.get_mut(id) {
                        worker.connected = false;
                    }
                    debug!("Worker {} disconnected", id);
                }
                break;
            }
            Ok(_) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                
                debug!("Received message: {}", line);
                
                if let Ok(message) = parse_worker_message(line) {
                    let id = format!("{}:{}", 
                        message.tool, 
                        message.directory.as_deref().unwrap_or("unknown")
                    );
                    
                    // Update worker state
                    {
                        let mut workers_guard = workers.write().await;
                        let worker = workers_guard.entry(id.clone()).or_insert_with(|| {
                            WorkerState {
                                tool: message.tool.clone(),
                                directory: message.directory.clone(),
                                status: "unknown".to_string(),
                                last_message: None,
                                bytes_transferred: None,
                                host: None,
                                pid: None,
                                connected: true,
                            }
                        });
                        
                        match message.message_type {
                            MessageType::Hello => {
                                worker.host = message.host.clone();
                                worker.pid = message.pid;
                                worker.connected = true;
                                worker.status = "connected".to_string();
                            }
                            MessageType::Start => {
                                worker.status = "running".to_string();
                            }
                            MessageType::Progress => {
                                worker.last_message = message.message.clone();
                                if let Some(bytes) = message.bytes {
                                    worker.bytes_transferred = Some(bytes);
                                }
                            }
                            MessageType::Stop => {
                                worker.status = message.status.clone().unwrap_or_else(|| "completed".to_string());
                                if let Some(bytes) = message.bytes {
                                    worker.bytes_transferred = Some(bytes);
                                }
                            }
                        }
                    }
                    
                    worker_id = Some(id);
                    
                    // Broadcast message to TUI clients
                    let _ = message_tx.send(message);
                } else {
                    warn!("Failed to parse worker message: {}", line);
                }
            }
            Err(e) => {
                error!("Error reading from worker connection: {}", e);
                break;
            }
        }
    }
    
    Ok(())
}

fn parse_worker_message(line: &str) -> Result<WorkerMessage> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    
    if parts.is_empty() {
        return Err(anyhow::anyhow!("Empty message"));
    }
    
    match parts[0] {
        "HELLO" => {
            // HELLO freight/0.1.0 host=hostname pid=1234
            let mut host = None;
            let mut pid = None;
            
            for part in &parts[2..] {
                if let Some(value) = part.strip_prefix("host=") {
                    host = Some(value.to_string());
                } else if let Some(value) = part.strip_prefix("pid=") {
                    pid = value.parse().ok();
                }
            }
            
            Ok(WorkerMessage {
                message_type: MessageType::Hello,
                tool: "unknown".to_string(),
                directory: None,
                status: None,
                bytes: None,
                message: None,
                host,
                pid,
            })
        }
        "START" => {
            // START tool=scan dir=user/
            let mut tool = "unknown".to_string();
            let mut directory = None;
            
            for part in &parts[1..] {
                if let Some(value) = part.strip_prefix("tool=") {
                    tool = value.to_string();
                } else if let Some(value) = part.strip_prefix("dir=") {
                    directory = Some(value.to_string());
                }
            }
            
            Ok(WorkerMessage {
                message_type: MessageType::Start,
                tool,
                directory,
                status: None,
                bytes: None,
                message: None,
                host: None,
                pid: None,
            })
        }
        "PROGRESS" => {
            // PROGRESS tool=scan dir=user/ msg=scanning...
            let mut tool = "unknown".to_string();
            let mut directory = None;
            let mut message = None;
            let mut bytes = None;
            
            for part in &parts[1..] {
                if let Some(value) = part.strip_prefix("tool=") {
                    tool = value.to_string();
                } else if let Some(value) = part.strip_prefix("dir=") {
                    directory = Some(value.to_string());
                } else if let Some(value) = part.strip_prefix("msg=") {
                    message = Some(value.to_string());
                } else if let Some(value) = part.strip_prefix("bytes=") {
                    bytes = value.parse().ok();
                }
            }
            
            Ok(WorkerMessage {
                message_type: MessageType::Progress,
                tool,
                directory,
                status: None,
                bytes,
                message,
                host: None,
                pid: None,
            })
        }
        "STOP" => {
            // STOP tool=scan dir=user/ status=ok bytes=1234 msg=completed
            let mut tool = "unknown".to_string();
            let mut directory = None;
            let mut status = None;
            let mut bytes = None;
            let mut message = None;
            
            for part in &parts[1..] {
                if let Some(value) = part.strip_prefix("tool=") {
                    tool = value.to_string();
                } else if let Some(value) = part.strip_prefix("dir=") {
                    directory = Some(value.to_string());
                } else if let Some(value) = part.strip_prefix("status=") {
                    status = Some(value.to_string());
                } else if let Some(value) = part.strip_prefix("bytes=") {
                    bytes = value.parse().ok();
                } else if let Some(value) = part.strip_prefix("msg=") {
                    message = Some(value.to_string());
                }
            }
            
            Ok(WorkerMessage {
                message_type: MessageType::Stop,
                tool,
                directory,
                status,
                bytes,
                message,
                host: None,
                pid: None,
            })
        }
        _ => Err(anyhow::anyhow!("Unknown message type: {}", parts[0])),
    }
}