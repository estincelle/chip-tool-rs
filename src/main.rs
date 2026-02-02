use axum::extract::connect_info::ConnectInfo;
use axum::{
    Router,
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    routing::any,
};
use axum_extra::TypedHeader;
use clap::{Parser, Subcommand};
use futures_util::stream::StreamExt;
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
    },
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
            InteractiveMode::Server { port } => {
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

    let (_, mut receiver) = socket.split();

    // Process messages from the client
    while let Some(msg_result) = receiver.next().await {
        match msg_result {
            Ok(msg) => {
                match msg {
                    Message::Text(text) => {
                        // Print the message content to stdout (mimicking chip-tool behavior)
                        println!("[{}] Message received: {}", who, text);
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
