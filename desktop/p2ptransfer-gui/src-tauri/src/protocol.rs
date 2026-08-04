use iroh::endpoint::Connection;
use iroh::protocol::{ProtocolHandler, AcceptError};
use p2ptransfer_core::network::alpn::{TransferEvent, TransferProtocol as CoreTransferProtocol, PendingRequests as CorePendingRequests};

pub use p2ptransfer_core::network::alpn::{
    ALPN, 
    TransferProgressEvent, 
    TransferErrorEvent, 
    SendStatusEvent, 
    ActiveTransferState,
    ACTIVE_TRANSFERS, 
    TRANSFER_PAUSE_FLAGS,
    PauseFlag,
    IncomingTransferEvent,
    TransferCompleteEvent
};

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::{RwLock, oneshot};
use tauri::{AppHandle, Manager};

/// Pending incoming transfer requests.
#[derive(Debug, Clone)]
pub struct PendingRequests(CorePendingRequests);

impl PendingRequests {
    pub fn new() -> Self {
        Self(CorePendingRequests::new())
    }
    
    pub fn core(&self) -> &CorePendingRequests {
        &self.0
    }
    
    pub async fn pending_ids(&self) -> Vec<String> {
        self.0.pending_ids().await
    }
    
    pub async fn register(&self, id: String) -> oneshot::Receiver<bool> {
        self.0.register(id).await
    }
    
    pub async fn respond(&self, id: &str, accepted: bool) -> bool {
        self.0.respond(id, accepted).await
    }
}

/// The protocol handler for incoming transfers, delegating to core.
#[derive(Debug, Clone)]
pub struct TransferProtocol {
    pub pending: PendingRequests,
    pub app_handle: AppHandle,
    pub output_dir: Arc<RwLock<String>>,
}

impl ProtocolHandler for TransferProtocol {
    fn accept(
        &self,
        connection: Connection,
    ) -> impl Future<Output = Result<(), AcceptError>> + Send {
        let (tx, mut rx) = tokio::sync::mpsc::channel(100);
        let app_handle = self.app_handle.clone();
        
        // Spawn a task to forward events to the Tauri frontend
        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                match event {
                    TransferEvent::SendStatus(e) => { let _ = app_handle.emit_all("send-status", e); },
                    TransferEvent::Rejected(e) => { let _ = app_handle.emit_all("transfer-rejected", e); },
                    TransferEvent::Error(e) => { let _ = app_handle.emit_all("transfer-error", e); },
                    TransferEvent::Incoming(e) => {
                        let _ = app_handle.emit_all("transfer-incoming", e.clone());
                        crate::open_receive_overlay(e.request_id, app_handle.clone());
                    },
                    TransferEvent::Progress(e) => { let _ = app_handle.emit_all("transfer-progress", e); },
                    TransferEvent::Complete(e) => { let _ = app_handle.emit_all("transfer-complete", e); },
                    TransferEvent::Cancelled { request_id: _ } => { let _ = app_handle.emit_all("transfer-cancelled", ()); },
                }
            }
        });
        
        // Use the core TransferProtocol to accept the connection
        let core_protocol = CoreTransferProtocol {
            pending: self.pending.0.clone(),
            event_tx: tx,
            output_dir: self.output_dir.clone(),
        };
        
        core_protocol.accept(connection)
    }
}

/// Helper to wrap the core send function and pipe events to Tauri.
pub async fn send_file_to_peer(
    request_id: String,
    endpoint: &iroh::Endpoint,
    target_node: iroh::EndpointAddr,
    file_path: &std::path::Path,
    sender_name: &str,
    app_handle: &AppHandle,
) -> anyhow::Result<String> {
    let (tx, mut rx) = tokio::sync::mpsc::channel(100);
    let ah = app_handle.clone();
    
    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            match event {
                TransferEvent::SendStatus(e) => { let _ = ah.emit_all("send-status", e); },
                TransferEvent::Rejected(e) => { let _ = ah.emit_all("transfer-rejected", e); },
                TransferEvent::Error(e) => { let _ = ah.emit_all("transfer-error", e); },
                TransferEvent::Incoming(e) => { let _ = ah.emit_all("transfer-incoming", e); },
                TransferEvent::Progress(e) => { let _ = ah.emit_all("send-progress", e); },
                TransferEvent::Complete(e) => { let _ = ah.emit_all("transfer-complete", e); },
                TransferEvent::Cancelled { request_id: _ } => { let _ = ah.emit_all("transfer-cancelled", ()); },
            }
        }
    });

    p2ptransfer_core::network::alpn::send_file_to_peer(
        request_id,
        endpoint,
        target_node,
        file_path,
        sender_name,
        &tx,
        false
    ).await
}
