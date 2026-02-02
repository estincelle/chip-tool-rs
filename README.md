# chip-tool-rs

A Rust implementation of `chip-tool interactive server` that provides a WebSocket server for receiving and responding to Matter commands.

## Features

- CLI interface using Clap that emulates `chip-tool interactive server`
- WebSocket server listening on port 9002 (configurable)
- JSON command parsing with base64-encoded arguments
- Realistic response generation matching chip-tool's format
- Support for the `delay` cluster's `wait-for-commissionee` command
- Connection logging and management
- **File logging** - All logs written to `chip-tool-rs.log` next to the binary

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

**Note**: The server also accepts messages with a `json:` prefix (used by the YAML test runner):
```
json:{"cluster": "delay", "command": "wait-for-commissionee", ...}
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

#### `onoff read`

Simulates reading the on-off attribute from a Matter device.

**Example Request:**
```json
{
  "cluster": "onoff",
  "command": "read",
  "arguments": "base64:eyJkZXN0aW5hdGlvbi1pZCI6ICIweDEyMzQ0MzIxIiwgImVuZHBvaW50LWlkcyI6ICIxIn0=",
  "command_specifier": "on-off"
}
```

The base64 string decodes to:
```json
{ "destination-id": "0x12344321", "endpoint-ids": "1" }
```

**Success Response:**
```json
{
  "results": [
    {
      "clusterId": 6,
      "endpointId": 1,
      "attributeId": 0,
      "value": true
    }
  ],
  "logs": [
    {
      "module": "chipTool",
      "category": "Info",
      "message": "UmVhZCBPbk9mZiBhdHRyaWJ1dGUgZnJvbSBlbmRwb2ludCAxOiBPTg=="
    }
  ]
}
```

**Note**: 
- `clusterId` is numeric (OnOff cluster = 6 = 0x0006)
- `endpointId` is numeric (parsed from string argument)
- `attributeId` is numeric (on-off attribute = 0)
- `value` is the boolean state

The log message is base64-encoded. Decoded: `"Read OnOff attribute from endpoint 1: ON"`

### Error Response Format

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

Test scripts are included:

1. **test_client.py** - Basic WebSocket connectivity test
2. **test_wait_for_commissionee.py** - Tests the wait-for-commissionee command
3. **test_onoff_read.py** - Tests the onoff read command
4. **test_json_prefix.py** - Tests json: prefix handling (YAML test runner compatibility)
5. **test_error_case.py** - Tests error handling

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

## Logging

The server logs to two destinations:

1. **Console (stdout)** - Real-time output for monitoring
2. **Log file** - Persistent logs written to `chip-tool-rs.log` in the same directory as the binary

### Log File Location

- **Development**: `target/debug/chip-tool-rs.log` or `target/release/chip-tool-rs.log`
- **Installed Binary**: Same directory as the `chip-tool-rs` executable

On startup, the server prints the log file location:
```
Logging to file: /path/to/chip-tool-rs.log
```

### Setting Log Level

Use the `RUST_LOG` environment variable to control log verbosity:

```bash
# Info level (default)
cargo run -- interactive server

# Debug level for detailed logging
RUST_LOG=chip_tool_rs=debug cargo run -- interactive server

# Trace level for maximum verbosity
RUST_LOG=chip_tool_rs=trace cargo run -- interactive server
```

## Server Output

When a client connects and sends commands, the server will print to both console and log file:

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
