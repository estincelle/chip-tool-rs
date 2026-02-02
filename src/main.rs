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
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

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
    #[serde(rename = "command_specifier")]
    command_specifier: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WaitForCommissioneeArgs {
    #[serde(rename = "nodeId")]
    node_id: String,
}

#[derive(Debug, Deserialize)]
struct OnOffReadArgs {
    #[serde(rename = "destination-id")]
    destination_id: String,
    #[serde(rename = "endpoint-ids")]
    endpoint_ids: String,
}

#[derive(Debug, Deserialize)]
struct OnOffWriteArgs {
    #[serde(rename = "destination-id")]
    destination_id: String,
    #[serde(rename = "endpoint-id-ignored-for-group-commands")]
    endpoint_id: String,
    #[serde(rename = "attribute-values")]
    attribute_values: String,
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
    // Get the directory of the executing binary
    let exe_path = std::env::current_exe()?;
    let exe_dir = exe_path
        .parent()
        .ok_or("Failed to get executable directory")?;

    // Create log file appender in the same directory as the binary
    let log_file_name = "chip-tool-rs.log";
    let file_appender = tracing_appender::rolling::never(exe_dir, log_file_name);
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    // Set up tracing with both console and file output
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| format!("{}=info,tower_http=info", env!("CARGO_CRATE_NAME")).into());

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt::layer().with_writer(std::io::stdout)) // Console output
        .with(fmt::layer().with_writer(non_blocking).with_ansi(false)) // File output
        .init();

    let log_path = exe_dir.join(log_file_name);
    tracing::info!("Logging to file: {}", log_path.display());

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

    tracing::info!("== WebSocket Server Ready");
    tracing::info!("WebSocket server listening on {}", addr);
    tracing::info!("Waiting for connections...");

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
    tracing::info!("Connection established with {}", who);

    let (mut sender, mut receiver) = socket.split();

    // Process messages from the client
    while let Some(msg_result) = receiver.next().await {
        match msg_result {
            Ok(msg) => {
                match msg {
                    Message::Text(text) => {
                        tracing::info!("[{}] Message received: {}", who, text);

                        // Process the command and generate response
                        if let Some(response) = process_command(&text) {
                            tracing::info!("[{}] Sending response: {}", who, response);
                            if sender.send(Message::Text(response.into())).await.is_err() {
                                tracing::error!("[{}] Failed to send response", who);
                                break;
                            }
                        }
                    }
                    Message::Binary(data) => {
                        tracing::info!(
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
                            tracing::info!(
                                "[{}] Connection closed: code={}, reason={}",
                                who,
                                cf.code,
                                cf.reason
                            );
                        } else {
                            tracing::info!("[{}] Connection closed", who);
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

    tracing::info!("Connection terminated with {}", who);
}

/// Process incoming commands and generate realistic chip-tool responses
fn process_command(message: &str) -> Option<String> {
    // Strip the "json:" prefix if present (used by YAML test runner)
    let json_message = message.strip_prefix("json:").unwrap_or(message);

    // Parse the command message
    let cmd: CommandMessage = match serde_json::from_str(json_message) {
        Ok(cmd) => cmd,
        Err(e) => {
            tracing::error!("Failed to parse command JSON: {}", e);
            return Some(create_error_response("Invalid JSON format"));
        }
    };

    tracing::info!(
        "Processing command: cluster={}, command={}{}",
        cmd.cluster,
        cmd.command,
        cmd.command_specifier
            .as_ref()
            .map(|s| format!(" (specifier: {})", s))
            .unwrap_or_default()
    );

    // Handle different cluster/command combinations
    match (cmd.cluster.to_lowercase().as_str(), cmd.command.as_str()) {
        ("delay", "wait-for-commissionee") => handle_wait_for_commissionee(&cmd.arguments),
        ("onoff", "read") => handle_onoff_read(&cmd.arguments),
        ("onoff", "write") => handle_onoff_write(&cmd.arguments, &cmd.command_specifier),
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

    tracing::info!("Decoded arguments: {}", decoded_args);

    // Parse the decoded arguments
    let args: WaitForCommissioneeArgs = match serde_json::from_str(&decoded_args) {
        Ok(args) => args,
        Err(e) => {
            tracing::error!("Failed to parse arguments JSON: {}", e);
            return Some(create_error_response("Invalid arguments format"));
        }
    };

    tracing::info!("Waiting for commissionee with nodeId: {}", args.node_id);

    // Simulate a successful connection to the device
    Some(create_success_response(&args.node_id))
}

/// Handle the onoff read command
fn handle_onoff_read(arguments: &str) -> Option<String> {
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

    tracing::debug!("Decoded arguments: {}", decoded_args);

    // Parse the decoded arguments
    let args: OnOffReadArgs = match serde_json::from_str(&decoded_args) {
        Ok(args) => args,
        Err(e) => {
            tracing::error!("Failed to parse arguments JSON: {}", e);
            return Some(create_error_response("Invalid arguments format"));
        }
    };

    tracing::info!(
        "Reading onoff attribute for destination: {}, endpoint: {}",
        args.destination_id,
        args.endpoint_ids
    );

    // Simulate reading the on-off attribute (returning "on" state)
    Some(create_onoff_read_response(
        &args.destination_id,
        &args.endpoint_ids,
        true,
    ))
}

/// Handle the onoff write command
fn handle_onoff_write(arguments: &str, command_specifier: &Option<String>) -> Option<String> {
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

    tracing::debug!("Decoded arguments: {}", decoded_args);

    // Parse the decoded arguments
    let args: OnOffWriteArgs = match serde_json::from_str(&decoded_args) {
        Ok(args) => args,
        Err(e) => {
            tracing::error!("Failed to parse arguments JSON: {}", e);
            return Some(create_error_response("Invalid arguments format"));
        }
    };

    let attribute_name = command_specifier.as_deref().unwrap_or("unknown");

    tracing::info!(
        "Writing onoff attribute '{}' for destination: {}, endpoint: {}, value: {}",
        attribute_name,
        args.destination_id,
        args.endpoint_id,
        args.attribute_values
    );

    // Simulate writing the attribute
    Some(create_onoff_write_response(
        &args.destination_id,
        &args.endpoint_id,
        attribute_name,
        &args.attribute_values,
    ))
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

/// Create a response for onoff read command
fn create_onoff_read_response(destination_id: &str, endpoint_id: &str, on_state: bool) -> String {
    let log_message = format!(
        "Read OnOff attribute from endpoint {}: {}",
        endpoint_id,
        if on_state { "ON" } else { "OFF" }
    );
    let encoded_log = BASE64.encode(log_message.as_bytes());

    // OnOff cluster ID is 0x0006 (6 in decimal)
    // Parse endpoint_id as integer, default to 1 if parsing fails
    let endpoint_num: u16 = endpoint_id.parse().unwrap_or(1);

    // Create a result object with the attribute value
    // Format matches chip-tool's actual response format
    let result = serde_json::json!({
        "clusterId": 6,
        "endpointId": endpoint_num,
        "attributeId": 0,  // on-off attribute ID is 0
        "value": on_state
    });

    let response = ResponseMessage {
        results: vec![result],
        logs: vec![LogEntry {
            module: "chipTool".to_string(),
            category: "Info".to_string(),
            message: encoded_log,
        }],
    };

    serde_json::to_string(&response).unwrap_or_else(|_| {
        r#"{"results":[{"error":"FAILURE"}],"logs":[{"module":"chipTool","category":"Error","message":"RmFpbGVkIHRvIHNlcmlhbGl6ZSByZXNwb25zZQ=="}]}"#.to_string()
    })
}

/// Create a response for onoff write command
fn create_onoff_write_response(
    destination_id: &str,
    endpoint_id: &str,
    attribute_name: &str,
    value: &str,
) -> String {
    let log_message = format!(
        "Write OnOff attribute '{}' to endpoint {}: value={}",
        attribute_name, endpoint_id, value
    );
    let encoded_log = BASE64.encode(log_message.as_bytes());

    // OnOff cluster ID is 0x0006 (6 in decimal)
    // Parse endpoint_id as integer, default to 1 if parsing fails
    let endpoint_num: u16 = endpoint_id.parse().unwrap_or(1);

    // Map attribute name to attribute ID
    // on-time is attribute 0x4001 (16385 in decimal)
    let attribute_id = match attribute_name {
        "on-time" => 16385,
        "off-wait-time" => 16386,
        _ => 0,
    };

    // Create a result object for the write operation
    // For successful writes, only return clusterId, endpointId, and attributeId
    // (no status or error field - absence of error indicates success)
    let result = serde_json::json!({
        "clusterId": 6,
        "endpointId": endpoint_num,
        "attributeId": attribute_id
    });

    let response = ResponseMessage {
        results: vec![result],
        logs: vec![LogEntry {
            module: "chipTool".to_string(),
            category: "Info".to_string(),
            message: encoded_log,
        }],
    };

    serde_json::to_string(&response).unwrap_or_else(|_| {
        r#"{"results":[{"error":"FAILURE"}],"logs":[{"module":"chipTool","category":"Error","message":"RmFpbGVkIHRvIHNlcmlhbGl6ZSByZXNwb25zZQ=="}]}"#.to_string()
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
