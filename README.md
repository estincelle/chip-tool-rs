# chip-tool-rs

A Rust implementation of `chip-tool interactive server` that provides a WebSocket server for receiving and responding to Matter commands.

## Features

- CLI interface using Clap that emulates `chip-tool interactive server`
- WebSocket server listening on port 9002 (configurable)
- JSON command parsing with base64-encoded arguments
- Realistic response generation matching chip-tool's format
- Support for the `delay` cluster's `wait-for-commissionee` command
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

## Command Protocol

### Message Format

Send JSON messages to the WebSocket server in this format:

```json
{
  "cluster": "cluster-name",
  "command": "command-name",
  "arguments": "base64:encoded_json_arguments"
}
```

### Supported Commands

#### `delay wait-for-commissionee`

Simulates establishing a connection with a Matter device.

**Example Request:**
```json
{
  "cluster": "delay",
  "command": "wait-for-commissionee",
  "arguments": "base64:eyJub2RlSWQiOiAiMzA1NDE0OTQ1In0="
}
```

The base64 string decodes to:
```json
{ "nodeId": "305414945" }
```

**Success Response:**
```json
{
  "results": [],
  "logs": [
    {
      "module": "chipTool",
      "category": "Info",
      "message": "RGV2aWNlIDMwNTQxNDk0NSBjb25uZWN0ZWQgc3VjY2Vzc2Z1bGx5"
    }
  ]
}
```

The log message is base64-encoded. Decoded: `"Device 305414945 connected successfully"`

**Error Response:**
```json
{
  "results": [
    {"error": "FAILURE"}
  ],
  "logs": [
    {
      "module": "chipTool",
      "category": "Error",
      "message": "base64_encoded_error_message"
    }
  ]
}
```

## Testing

### Test Scripts

Three test scripts are included:

1. **test_client.py** - Basic WebSocket connectivity test
2. **test_wait_for_commissionee.py** - Tests the wait-for-commissionee command
3. **test_error_case.py** - Tests error handling

### Running Tests

```bash
# In one terminal, start the server
cargo run -- interactive server

# In another terminal, run a test
python3 test_wait_for_commissionee.py
```

### Example Test Output

```
Connected to ws://localhost:9002

=== Sending Command ===
{
  "cluster": "delay",
  "command": "wait-for-commissionee",
  "arguments": "base64:eyJub2RlSWQiOiAiMzA1NDE0OTQ1In0="
}

Command sent, waiting for response...

=== Received Response ===
{"results":[],"logs":[{"module":"chipTool","category":"Info","message":"RGV2aWNlIDMwNTQxNDk0NSBjb25uZWN0ZWQgc3VjY2Vzc2Z1bGx5"}]}

=== Decoded Log Messages ===
[Info] Device 305414945 connected successfully

✅ Command SUCCESSFUL
```

## Server Output

When a client connects and sends commands, the server will print:

```
== WebSocket Server Ready
WebSocket server listening on 0.0.0.0:9002
Waiting for connections...
Client connected: Python/3.12 websockets/12.0 from 127.0.0.1:62241
Connection established with 127.0.0.1:62241
[127.0.0.1:62241] Message received: {"cluster": "delay", "command": "wait-for-commissionee", ...}
Processing command: cluster=delay, command=wait-for-commissionee
Decoded arguments: {"nodeId": "305414945"}
Waiting for commissionee with nodeId: 305414945
[127.0.0.1:62241] Sending response: {"results":[],"logs":[...]}
[127.0.0.1:62241] Connection closed: code=1000, reason=
Connection terminated with 127.0.0.1:62241
```

## Architecture

The implementation uses:
- **Clap**: For CLI argument parsing
- **Axum**: For HTTP/WebSocket server framework
- **Tokio**: For async runtime
- **Serde**: For JSON serialization/deserialization
- **Base64**: For argument encoding/decoding
- **Tower-HTTP**: For HTTP middleware and tracing

## Response Format

Responses follow the chip-tool interactive server format:

```json
{
  "results": [
    // Array of result objects (empty on success)
    // Contains {"error": "FAILURE"} on error
  ],
  "logs": [
    {
      "module": "chipTool",
      "category": "Info|Error|Debug",
      "message": "base64_encoded_log_message"
    }
  ]
}
```

All log messages are base64-encoded to match chip-tool's behavior.

## Comparison with chip-tool

This implementation emulates the behavior of the original chip-tool's interactive server mode:
- Same default port (9002)
- WebSocket-based communication
- JSON command format with base64-encoded arguments
- Structured JSON responses with base64-encoded logs
- Configurable port via command-line argument

The original chip-tool uses libwebsockets and implements a full Matter commissioning and interaction tool. This Rust version provides stubbed responses that match chip-tool's format, making it suitable for testing and development of clients that interact with chip-tool.

## Documentation

See `docs/CHIP-TOOL_BEHAVIOR.md` for detailed documentation of chip-tool's interactive server behavior, including:
- WebSocket protocol details
- Command processing flow
- The `--trace-decode` option
- Complete example of the `wait-for-commissionee` command

## License

[Add your license here]
