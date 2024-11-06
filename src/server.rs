use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use futures_util::{StreamExt, SinkExt};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

type Rooms = Arc<Mutex<HashMap<String, broadcast::Sender<String>>>>;

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

#[tokio::main]
async fn main() {
    let rooms: Rooms = Arc::new(Mutex::new(HashMap::new()));
    let listener = TcpListener::bind("127.0.0.1:8080").await.expect("Can't bind");

    println!("Server is running on 127.0.0.1:8080");

    while let Ok((stream, _)) = listener.accept().await {
        let rooms = rooms.clone();
        tokio::spawn(handle_connection(stream, rooms));
    }
}
