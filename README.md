# chip-tool-rs

A Rust implementation of `chip-tool`.

## Features

- CLI interface using Clap that emulates `chip-tool interactive server`
- WebSocket server listening on port 9002 (configurable)
- Prints all received messages to stdout
- Connection logging and management

## Usage

### Build

```bash
cargo build --release
```

### Run the Server

Start the server with default port (9002):

```bash
cargo run -- interactive server
```

Or specify a custom port:

```bash
cargo run -- interactive server --port 8080
```

### Help

```bash
cargo run -- --help
cargo run -- interactive server --help
```

## Testing

A test client script is included to verify functionality:

```bash
# In one terminal, start the server
cargo run -- interactive server

# In another terminal, run the test client
python3 test_client.py
```

## Server Output

When a client connects and sends messages, the server will print:

```
WebSocket server listening on 0.0.0.0:9002
Waiting for connections...
Client connected: Python/3.12 websockets/12.0 from 127.0.0.1:64823
Connection established with 127.0.0.1:64823
[127.0.0.1:64823] Message received: Hello from test client
[127.0.0.1:64823] Message received: This is a test message
[127.0.0.1:64823] Connection closed: code=1000, reason=
Connection terminated with 127.0.0.1:64823
```

## Architecture

The implementation uses:
- **Clap**: For CLI argument parsing
- **Axum**: For HTTP/WebSocket server framework
- **Tokio**: For async runtime
- **Tower-HTTP**: For HTTP middleware and tracing

## Comparison with chip-tool

This implementation emulates the behavior of the original chip-tool's interactive server mode:
- Same default port (9002)
- WebSocket-based communication
- Message content printed to stdout
- Configurable port via command-line argument

The original chip-tool uses libwebsockets and implements a full Matter commissioning and interaction tool. This Rust version focuses specifically on the WebSocket server functionality for receiving and displaying messages.
