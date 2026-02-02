use axum::extract::connect_info::ConnectInfo;
use axum::{
    Router,
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    routing::any,
};
use axum_extra::TypedHeader;
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use clap::{Parser, Subcommand};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tower_http::trace::{DefaultMakeSpan, TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// A Rust implementation of chip-tool's interactive server
#[derive(Parser)]
#[command(name = "chip-tool")]
#[command(about = "A Rust implementation of chip-tool's interactive server", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Interactive mode commands
    Interactive {
        #[command(subcommand)]
        mode: InteractiveMode,
    },
}

#[derive(Subcommand)]
enum InteractiveMode {
    /// Start the interactive server mode
    Server {
        /// Port the websocket will listen to. Defaults to 9002.
        #[arg(long, default_value_t = 9002)]
        port: u16,
        /// Enable tracing of all exchanged messages. 0 = off, 1 = on
        #[arg(long = "trace_decode")]
        trace_decode: Option<u8>,
    },
}

// Message structures matching chip-tool's format
#[derive(Debug, Deserialize)]
struct CommandMessage {
    cluster: String,
    command: String,
    arguments: String,
}

#[derive(Debug, Deserialize)]
struct WaitForCommissioneeArgs {
    #[serde(rename = "nodeId")]
    node_id: String,
}

#[derive(Debug, Serialize)]
struct ResponseMessage {
    results: Vec<serde_json::Value>,
    logs: Vec<LogEntry>,
}

#[derive(Debug, Serialize)]
struct LogEntry {
    module: String,
    category: String,
    message: String,
}

#[derive(Debug, Serialize)]
struct ErrorResult {
    error: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                format!("{}=info,tower_http=info", env!("CARGO_CRATE_NAME")).into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Interactive { mode } => match mode {
            InteractiveMode::Server { port, .. } => {
                run_server(port).await?;
            }
        },
    }

    Ok(())
}

async fn run_server(port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let app = Router::new().route("/", any(ws_handler)).layer(
        TraceLayer::new_for_http().make_span_with(DefaultMakeSpan::default().include_headers(true)),
    );

    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    println!("== WebSocket Server Ready");
    println!("WebSocket server listening on {}", addr);
    println!("Waiting for connections...");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
}

/// The handler for the HTTP request (this gets called when the HTTP request lands at the start
/// of websocket negotiation). After this completes, the actual switching from HTTP to
/// websocket protocol will occur.
async fn ws_handler(
    ws: WebSocketUpgrade,
    user_agent: Option<TypedHeader<headers::UserAgent>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> impl IntoResponse {
    let user_agent = if let Some(TypedHeader(user_agent)) = user_agent {
        user_agent.to_string()
    } else {
        String::from("Unknown")
    };

    tracing::info!("Client connected: {} from {}", user_agent, addr);

    ws.on_upgrade(move |socket| handle_socket(socket, addr))
}

/// Actual websocket state machine (one will be spawned per connection)
async fn handle_socket(socket: WebSocket, who: SocketAddr) {
    println!("Connection established with {}", who);

    let (mut sender, mut receiver) = socket.split();

    // Process messages from the client
    while let Some(msg_result) = receiver.next().await {
        match msg_result {
            Ok(msg) => {
                match msg {
                    Message::Text(text) => {
                        println!("[{}] Message received: {}", who, text);

                        // Process the command and generate response
                        if let Some(response) = process_command(&text) {
                            println!("[{}] Sending response: {}", who, response);
                            if sender.send(Message::Text(response.into())).await.is_err() {
                                tracing::error!("[{}] Failed to send response", who);
                                break;
                            }
                        }
                    }
                    Message::Binary(data) => {
                        println!(
                            "[{}] Binary message received ({} bytes): {:?}",
                            who,
                            data.len(),
                            data
                        );
                    }
                    Message::Ping(data) => {
                        tracing::debug!("[{}] Ping received: {:?}", who, data);
                    }
                    Message::Pong(data) => {
                        tracing::debug!("[{}] Pong received: {:?}", who, data);
                    }
                    Message::Close(close_frame) => {
                        if let Some(cf) = close_frame {
                            println!(
                                "[{}] Connection closed: code={}, reason={}",
                                who, cf.code, cf.reason
                            );
                        } else {
                            println!("[{}] Connection closed", who);
                        }
                        break;
                    }
                }
            }
            Err(e) => {
                tracing::error!("[{}] WebSocket error: {}", who, e);
                break;
            }
        }
    }

    println!("Connection terminated with {}", who);
}

/// Process incoming commands and generate realistic chip-tool responses
fn process_command(message: &str) -> Option<String> {
    // Parse the command message
    let cmd: CommandMessage = match serde_json::from_str(message) {
        Ok(cmd) => cmd,
        Err(e) => {
            tracing::error!("Failed to parse command JSON: {}", e);
            return Some(create_error_response("Invalid JSON format"));
        }
    };

    println!(
        "Processing command: cluster={}, command={}",
        cmd.cluster, cmd.command
    );

    // Handle different cluster/command combinations
    match (cmd.cluster.to_lowercase().as_str(), cmd.command.as_str()) {
        ("delay", "wait-for-commissionee") => handle_wait_for_commissionee(&cmd.arguments),
        _ => Some(create_error_response(&format!(
            "Unknown command: {} {}",
            cmd.cluster, cmd.command
        ))),
    }
}

/// Handle the wait-for-commissionee command
fn handle_wait_for_commissionee(arguments: &str) -> Option<String> {
    // Decode base64 arguments
    let decoded_args = if let Some(base64_data) = arguments.strip_prefix("base64:") {
        match BASE64.decode(base64_data) {
            Ok(data) => match String::from_utf8(data) {
                Ok(s) => s,
                Err(e) => {
                    tracing::error!("Failed to decode base64 as UTF-8: {}", e);
                    return Some(create_error_response("Invalid base64 encoding"));
                }
            },
            Err(e) => {
                tracing::error!("Failed to decode base64: {}", e);
                return Some(create_error_response("Invalid base64 format"));
            }
        }
    } else {
        tracing::error!("Arguments missing 'base64:' prefix");
        return Some(create_error_response("Arguments must be base64 encoded"));
    };

    println!("Decoded arguments: {}", decoded_args);

    // Parse the decoded arguments
    let args: WaitForCommissioneeArgs = match serde_json::from_str(&decoded_args) {
        Ok(args) => args,
        Err(e) => {
            tracing::error!("Failed to parse arguments JSON: {}", e);
            return Some(create_error_response("Invalid arguments format"));
        }
    };

    println!("Waiting for commissionee with nodeId: {}", args.node_id);

    // Simulate a successful connection to the device
    Some(create_success_response(&args.node_id))
}

/// Create a success response for wait-for-commissionee
fn create_success_response(node_id: &str) -> String {
    let log_message = format!("Device {} connected successfully", node_id);
    let encoded_log = BASE64.encode(log_message.as_bytes());

    let response = ResponseMessage {
        results: vec![],
        logs: vec![LogEntry {
            module: "chipTool".to_string(),
            category: "Info".to_string(),
            message: encoded_log,
        }],
    };

    serde_json::to_string(&response).unwrap_or_else(|_| {
        r#"{"results":[],"logs":[{"module":"chipTool","category":"Error","message":"RmFpbGVkIHRvIHNlcmlhbGl6ZSByZXNwb25zZQ=="}]}"#.to_string()
    })
}

/// Create an error response
fn create_error_response(error_msg: &str) -> String {
    let encoded_error = BASE64.encode(error_msg.as_bytes());

    let response = ResponseMessage {
        results: vec![serde_json::json!({"error": "FAILURE"})],
        logs: vec![LogEntry {
            module: "chipTool".to_string(),
            category: "Error".to_string(),
            message: encoded_error,
        }],
    };

    serde_json::to_string(&response).unwrap_or_else(|_| {
        r#"{"results":[{"error":"FAILURE"}],"logs":[{"module":"chipTool","category":"Error","message":"VW5rbm93biBlcnJvcg=="}]}"#.to_string()
    })
}
