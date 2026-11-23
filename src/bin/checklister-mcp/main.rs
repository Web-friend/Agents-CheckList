mod server;

use std::process::ExitCode;

use checklister::{Store, connect};
use rmcp::{ServiceExt, transport::stdio};

use crate::server::ChecklisterServer;

async fn run() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let store = Store::new(connect().await?);
    let service = ChecklisterServer::new(store).serve(stdio()).await?;
    let cancellation = service.cancellation_token();

    tokio::select! {
        result = service.waiting() => {
            result?;
        }
        _ = shutdown_signal() => {
            cancellation.cancel();
        }
    }

    Ok(())
}

#[cfg(unix)]
async fn shutdown_signal() {
    use tokio::signal::unix::{SignalKind, signal};

    let mut terminate = signal(SignalKind::terminate()).expect("install SIGTERM handler");
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = terminate.recv() => {}
    }
}

#[cfg(not(unix))]
async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("checklister-mcp: {error}");
            ExitCode::FAILURE
        }
    }
}
