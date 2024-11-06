use std::env;
use tokio::io::{self, AsyncBufReadExt};
use tokio::sync::mpsc;
use tokio_tungstenite::connect_async;
use futures_util::{SinkExt, StreamExt};

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    // Create bindings for default values
    let default_room = "default".to_string();
    let default_server = "ws://127.0.0.1:8080".to_string();

    // Use the bindings in unwrap_or
    let room_id = args.get(1).unwrap_or(&default_room);
    let server_addr = args.get(2).unwrap_or(&default_server);

    // Connect to the server
    let (mut ws_stream, _) = connect_async(server_addr).await.expect("Failed to connect");
    println!("Connected to the server!");

    // Send initial room join message
    ws_stream.send(tokio_tungstenite::tungstenite::Message::Text(format!("Joining room: {}", room_id)))
        .await
        .expect("Failed to send message");

    // Set up a channel to read user input asynchronously
    let (tx, mut rx) = mpsc::unbounded_channel();

    // Spawn a task to read from stdin
    tokio::spawn(async move {
        let stdin = io::BufReader::new(io::stdin());
        let mut lines = stdin.lines();

        while let Ok(Some(line)) = lines.next_line().await {
            if tx.send(line).is_err() {
                break;
            }
        }
    });

    // Loop to handle user input and send it to the server
    while let Some(message) = rx.recv().await {
        ws_stream.send(tokio_tungstenite::tungstenite::Message::Text(message))
            .await
            .expect("Failed to send message");
    }
}
