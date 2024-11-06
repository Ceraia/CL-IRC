use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use clap::Parser;
use tokio::net::{TcpListener};
use tokio_tungstenite::accept_async;
use futures_util::{stream::StreamExt, SinkExt};
use tokio::sync::broadcast;

#[derive(Parser)]
#[command(name = "server")]
struct Args {
    #[arg(short, long, default_value = "127.0.0.1")]
    ip: String,
    #[arg(short, long, default_value = "80")]
    port: u16,
}

type Rooms = Arc<Mutex<HashMap<String, broadcast::Sender<String>>>>;

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let rooms: Rooms = Arc::new(Mutex::new(HashMap::new()));

    let address = format!("{}:{}", args.ip, args.port);
    let listener = TcpListener::bind(&address).await.expect("Failed to bind");
    println!("Server listening on {}", address);

    while let Ok((stream, _)) = listener.accept().await {
        let rooms = rooms.clone();
        tokio::spawn(handle_connection(stream, rooms));
    }
}

async fn handle_connection(raw_stream: tokio::net::TcpStream, rooms: Rooms) {
    let ws_stream = accept_async(raw_stream).await.expect("Failed to accept connection");
    let (mut write, mut read) = ws_stream.split();

    // Join room and get broadcast channel
    let room_name = "default".to_string();
    let tx = rooms.lock().unwrap().entry(room_name.clone())
        .or_insert_with(|| {
            let (tx, _) = broadcast::channel(100);
            tx
        })
        .clone();

    // Clone tx for use in the async block
    let tx_clone = tx.clone();
    tokio::spawn(async move {
        while let Some(Ok(msg)) = read.next().await {
            if let Ok(text) = msg.into_text() {
                println!("Received: {}", text); // Log the message
                let _ = tx_clone.send(text);
            }
        }
    });

    // Use the original tx to create a subscriber
    let mut rx = tx.subscribe();
    while let Ok(msg) = rx.recv().await {
        if write.send(tokio_tungstenite::tungstenite::Message::Text(msg)).await.is_err() {
            break;
        }
    }
}