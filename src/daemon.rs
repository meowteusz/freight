use crate::{Config, SocketServer, WorkerManager};
use anyhow::Result;
use tracing::{info, error};
use tokio::signal;

pub async fn start_daemon() -> Result<()> {
    info!("Starting freight daemon");
    
    let (socket_server, message_rx) = SocketServer::new();
    let worker_manager = WorkerManager::new();
    
    // Start socket server
    let socket_handle = tokio::spawn(async move {
        if let Err(e) = socket_server.start().await {
            error!("Socket server error: {}", e);
        }
    });
    
    // Start worker manager
    let worker_handle = tokio::spawn(async move {
        worker_manager.start(message_rx).await;
    });
    
    // Wait for shutdown signal
    tokio::select! {
        _ = signal::ctrl_c() => {
            info!("Received shutdown signal");
        }
        _ = socket_handle => {
            error!("Socket server terminated unexpectedly");
        }
        _ = worker_handle => {
            error!("Worker manager terminated unexpectedly");
        }
    }
    
    // Cleanup
    let _ = std::fs::remove_file(crate::socket::SOCKET_PATH);
    info!("Freight daemon stopped");
    
    Ok(())
}

pub async fn start_migration_daemon(config: Config) -> Result<()> {
    info!("Starting freight migration daemon with config: {:?}", config);
    
    let (socket_server, message_rx) = SocketServer::new();
    let mut worker_manager = WorkerManager::new();
    
    // Set migration config
    worker_manager.set_config(config);
    
    // Start socket server
    let socket_handle = tokio::spawn(async move {
        if let Err(e) = socket_server.start().await {
            error!("Socket server error: {}", e);
        }
    });
    
    // Start worker manager with migration
    let worker_handle = tokio::spawn(async move {
        worker_manager.start_migration(message_rx).await;
    });
    
    // Wait for shutdown signal or completion
    tokio::select! {
        _ = signal::ctrl_c() => {
            info!("Received shutdown signal");
        }
        _ = socket_handle => {
            error!("Socket server terminated unexpectedly");
        }
        _ = worker_handle => {
            info!("Migration completed");
        }
    }
    
    // Cleanup
    let _ = std::fs::remove_file(crate::socket::SOCKET_PATH);
    info!("Freight migration daemon stopped");
    
    Ok(())
}

pub async fn daemonize_and_start() -> Result<()> {
    // For now, just run in foreground
    // In a full implementation, this would fork and detach
    info!("Daemonizing freight (running in foreground for now)");
    start_daemon().await
}