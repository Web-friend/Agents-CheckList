
mod storage;

use std::error::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

async fn handle_client(mut stream: TcpStream) -> Result<(), Box<dyn Error + Send + Sync>> {
    println!("New connection: {}", stream.peer_addr()?);
    let mut buffer = [0; 1024];
    loop {
        let bytes_read = stream.read(&mut buffer).await?;
        if bytes_read == 0 {
            return Ok(());
        }
        stream.write_all(&buffer[..bytes_read]).await?;
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let _storage = storage::connect().await?;
    let address = std::env::var("CHECKLISTER_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".into());
    let listener = TcpListener::bind(&address).await?;
    println!("TCP echo server listening on {address}");

    loop {
        let (stream, _) = listener.accept().await?;
        tokio::spawn(async move {
            if let Err(error) = handle_client(stream).await {
                eprintln!("Error handling client: {error}");
            }
        });
    }
}
