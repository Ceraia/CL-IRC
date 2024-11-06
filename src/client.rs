use std::env;
use uuid::Uuid;
use tokio::io::{self, AsyncBufReadExt};
use tokio_tungstenite::connect_async;
use futures_util::{stream::StreamExt, SinkExt};

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    // Create bindings for default values
    let default_room = "default".to_string();
    let default_server = "ws://127.0.0.1:8080".to_string();

    // Use the bindings in unwrap_or
    let room_id = args.get(1).unwrap_or(&default_room);
    let server_addr = args.get(2).unwrap_or(&default_server);

    // Generate a unique client ID
    let client_id = Uuid::new_v4().to_string();

    // Connect to the server
    let (ws_stream, _) = connect_async(server_addr).await.expect("Failed to connect");
    println!("Connected to the server!");

    // Split the WebSocket stream into a writer and reader
    let (mut write, mut read) = ws_stream.split();

    // Send initial room join message
    write
        .send(tokio_tungstenite::tungstenite::Message::Text(format!(
            "Joining room: {}",
            room_id
        )))
        .await
        .expect("Failed to send message");

    // Spawn a task to handle user input and send it to the server
    let client_id_clone = client_id.clone();
    tokio::spawn(async move {
        let stdin = io::BufReader::new(io::stdin());
        let mut lines = stdin.lines();

        while let Ok(Some(line)) = lines.next_line().await {
            if !line.trim().is_empty() {
                // Format the message with the client ID
                let message = format!("{}:{}", client_id_clone, line);

                // Send the message to the server
                if let Err(e) = write
                    .send(tokio_tungstenite::tungstenite::Message::Text(message))
                    .await
                {
                    eprintln!("Failed to send message: {}", e);
                    break;
                }
            }
        }
    });

    // Handle incoming messages from the server
    while let Some(Ok(msg)) = read.next().await {
        if let tokio_tungstenite::tungstenite::Message::Text(text) = msg {
            // Split the message into client ID and content
            let parts: Vec<&str> = text.splitn(2, ':').collect();
            if parts.len() == 2 {
                let sender_id = parts[0];
                let content = parts[1];

                // Only display the message if it was not sent by this client
                if sender_id != client_id {
                    println!("Received: {}", content);
                }
            }
        }
    }
}
