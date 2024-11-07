use clap::Parser;
use uuid::Uuid;
use tokio::io::{self, AsyncBufReadExt};
use tokio_tungstenite::connect_async;
use futures_util::{stream::StreamExt, SinkExt};
use whoami;
use ratatui::{Terminal, backend::CrosstermBackend, widgets::{Block, Borders, Paragraph}, layout::{Layout, Constraint, Direction}};
use std::io::stdout;
use crossterm::event::{self, Event, KeyCode};
use tokio::sync::Mutex;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Parser)]
#[command(name = "client")]
struct Args {
    #[arg(short, long, default_value = "default")]
    room: String,
    #[arg(short, long, default_value = "127.0.0.1")]
    ip: String,
    #[arg(short, long, default_value = "80")]
    port: u16,
    #[arg(short, long)]
    username: Option<String>,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    // Generate the WebSocket URL
    let server_url = format!("ws://{}:{}", args.ip, args.port);

    // Generate a unique client ID
    let client_id = Uuid::new_v4().to_string();

    // Get the username or use the OS's username if not provided
    let username = args.username.unwrap_or_else(|| whoami::username());

    // Connect to the server
    let (ws_stream, _) = connect_async(&server_url).await.expect("Failed to connect");
    println!("Connected to the server!");

    // Split the WebSocket stream into a writer and reader
    let (mut write, mut read) = ws_stream.split();

    // Prepare the terminal and UI state
    let mut stdout = stdout();
    crossterm::terminal::enable_raw_mode().expect("Failed to enable raw mode");
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).expect("Failed to initialize terminal");

    // Message history and user input
    let message_history = Arc::new(Mutex::new(vec![]));
    let input = Arc::new(Mutex::new(String::new()));

    // Track the last event to debounce duplicate inputs
    let mut last_input_event: Option<Instant> = None;

    // Spawn a task to handle user input and send it to the server
    let client_id_clone = client_id.clone();
    let username_clone = username.clone();
    let input_clone = input.clone();
    tokio::spawn(async move {
        loop {
            if event::poll(Duration::from_millis(100)).unwrap() {
                if let Event::Key(key) = event::read().unwrap() {
                    let now = Instant::now();
                    // Check debounce timing to avoid quick duplicate inputs
                    if last_input_event.map_or(true, |last| now.duration_since(last) > Duration::from_millis(50)) {
                        last_input_event = Some(now);

                        match key.code {
                            KeyCode::Enter => {
                                let mut input = input_clone.lock().await;
                                if !input.trim().is_empty() {
                                    let message = format!("{}:{}:{}", client_id_clone, username_clone, input.clone());
                                    write.send(tokio_tungstenite::tungstenite::Message::Text(message)).await.unwrap();
                                    input.clear();
                                }
                            }
                            KeyCode::Char(c) => input_clone.lock().await.push(c),
                            KeyCode::Backspace => { input_clone.lock().await.pop(); },
                            _ => {}
                        }
                    }
                }
            }
        }
    });

    // Handle incoming messages from the server
    let message_history_clone = message_history.clone();
    tokio::spawn(async move {
        while let Some(Ok(msg)) = read.next().await {
            if let tokio_tungstenite::tungstenite::Message::Text(text) = msg {
                let parts: Vec<&str> = text.splitn(3, ':').collect();
                if parts.len() == 3 {
                    let sender_id = parts[0];
                    let sender_username = parts[1];
                    let content = parts[2];

                    if sender_id != client_id {
                        let mut history = message_history_clone.lock().await;
                        history.push(format!("{}: {}", sender_username, content));
                    }
                }
            }
        }
    });

    // Main UI render loop with `Ctrl+C` handling
    let mut should_exit = false;
    let ctrl_c_signal = tokio::spawn(async {
        tokio::signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
    });

    loop {
        if ctrl_c_signal.is_finished() {
            should_exit = true;
        }

        if should_exit {
            break;
        }

        // Lock and read data before entering the draw call
        let history = {
            let history = message_history.lock().await;
            history.join("\n")
        };

        let input_text = {
            let input = input.lock().await;
            input.clone()
        };

        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(5), Constraint::Length(3)].as_ref())
                .split(f.size());

            // Display the message history
            let message_display = Paragraph::new(history.clone())
                .block(Block::default().title("Messages").borders(Borders::ALL));
            f.render_widget(message_display, chunks[0]);

            // Display the user input field
            let input_display = Paragraph::new(input_text.clone())
                .block(Block::default().title("Input").borders(Borders::ALL));
            f.render_widget(input_display, chunks[1]);
        }).expect("Failed to draw the UI");
    }

    // Restore the terminal
    crossterm::terminal::disable_raw_mode().expect("Failed to disable raw mode");
    println!("Exiting...");
}
