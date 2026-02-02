# chip-tool Interactive Server Behavior

This document details the behavior and implementation of chip-tool's interactive server mode, based on analysis of the source code at `SimuMatter/connectedhomeip/examples/chip-tool`.

## Table of Contents

1. [Interactive Server Mode Overview](#interactive-server-mode-overview)
2. [WebSocket Protocol](#websocket-protocol)
3. [The --trace-decode Option](#the---trace-decode-option)
4. [Command Processing Flow](#command-processing-flow)
5. [Example: wait-for-commissionee Command](#example-wait-for-commissionee-command)

---

## Interactive Server Mode Overview

### Architecture

The chip-tool interactive server is implemented through several key components:

**Main Files:**
- `commands/interactive/InteractiveCommands.h`
- `commands/interactive/InteractiveCommands.cpp`
- `commands/interactive/Commands.h`
- `examples/common/websocket-server/WebSocketServer.h`
- `examples/common/websocket-server/WebSocketServer.cpp`

### WebSocket Connection

The interactive server uses the **libwebsockets** library for WebSocket communication:

- **Default Port**: 9002 (configurable via `--port` argument)
- **Protocol**: WebSocket over TCP
- **Message Format**: JSON with optional base64 encoding for arguments

**Starting the Server:**
```bash
chip-tool interactive server [--port 9002]
```

### Key Implementation Details

- **WebSocketServer Class**: Wraps libwebsockets functionality
- **WebSocketServerDelegate Interface**: Handles received messages
- **Global WebSocket Instance**: `gWebSocketInstance` tracks active connections
- **Message Queue**: `gMessageQueue` implemented with `std::deque<std::string>` for thread-safe message sending
- **Mutex Protection**: `gMutex` for concurrent access from multiple threads

### Connection States Handled

| Callback | Purpose |
|----------|---------|
| `LWS_CALLBACK_ESTABLISHED` | Sets up the WebSocket instance |
| `LWS_CALLBACK_RECEIVE` | Handles incoming messages |
| `LWS_CALLBACK_SERVER_WRITEABLE` | Flushes queued messages to client |
| `LWS_CALLBACK_WSI_DESTROY` | Cleans up when connection closes |
| `LWS_CALLBACK_PROTOCOL_INIT` | Logs when server is ready |

---

## WebSocket Protocol

### Message Format

Messages sent to the chip-tool interactive server follow this JSON structure:

```json
{
  "cluster": "cluster-name",
  "command": "command-name",
  "arguments": "base64:encoded_json_arguments"
}
```

### Argument Encoding

Arguments are base64-encoded JSON objects with the prefix `"base64:"`:

**Example:**
```json
{
  "cluster": "delay",
  "command": "wait-for-commissionee",
  "arguments": "base64:eyAibm9kZUlkIjoiMzA1NDE0OTQ1IiB9"
}
```

The base64 string decodes to:
```json
{ "nodeId": "305414945" }
```

### Response Format

Responses are returned as JSON with the following structure:

```json
{
  "results": [
    { /* command results as JSON objects */ }
  ],
  "logs": [
    {
      "module": "string",
      "category": "Error|Info|Debug",
      "message": "base64-encoded-message"
    }
  ]
}
```

**Success Response:**
```json
{
  "results": [],
  "logs": [
    {
      "module": "chipTool",
      "category": "Info",
      "message": "U3VjY2Vzc2Z1bGx5IGNvbm5lY3RlZA=="
    }
  ]
}
```

**Failure Response:**
```json
{
  "results": [
    {"error": "FAILURE"}
  ],
  "logs": [
    {
      "module": "chipTool",
      "category": "Error",
      "message": "RXJyb3IgY29ubmVjdGluZw=="
    }
  ]
}
```

### Logging Categories

- `kCategoryError` - Error messages
- `kCategoryProgress` - Information/progress messages (mapped to "Info")
- `kCategoryDetail` - Debug messages
- `kCategoryAutomation` - Automation messages (filtered out for server)

### Async Reports

Special message types for long-running operations:

- **Empty String**: Triggers async mode (waits for subscription reports)
- **Numeric Timeout**: String containing only a number (e.g., "5000") triggers async mode with timeout in milliseconds

```cpp
bool isAsyncReport = strlen(msg) == 0;
uint16_t timeout = 0;
if (!isAsyncReport && strlen(msg) <= 5) {
    std::stringstream ss;
    ss << msg;
    ss >> timeout;
    if (!ss.fail()) {
        isAsyncReport = true;
    }
}
```

---

## The --trace-decode Option

### Overview

The `--trace-decode` (or `--trace_decode`) option is a **protocol debugging feature** that enables real-time decoding and human-readable interpretation of Matter protocol messages.

### Usage

```bash
chip-tool <command> --trace_decode 1
```

Or in interactive server mode:
```bash
chip-tool interactive server --trace_decode 1
```

### What It Does

When enabled, the trace decoder:

1. **Intercepts Protocol Messages** - Captures all Matter protocol messages sent and received
2. **Decodes Protocol Information**:
   - Message direction (inbound `<< from` / outbound `>> to`)
   - Peer address and network endpoint
   - Message counter (sequence numbers)
   - Protocol names and IDs
   - Security flags and encryption details
   - Message and exchange flags
   - Decrypted payload data in hexadecimal

3. **Supports Multiple Protocols**:
   - Secure Channel protocol
   - Interaction Model protocol
   - BDX (Bulk Data eXchange)
   - User-Directed Commissioning (UDC)
   - Echo protocol

### Implementation

**Location:** `commands/common/CHIPCommand.cpp:320-327`

```cpp
if (mTraceDecode.HasValue() && mTraceDecode.Value()) {
    chip::trace::TraceDecoderOptions options;
    // Interaction Model responses already logged, avoid duplication
    options.mEnableProtocolInteractionModelResponse = false;
    chip::trace::TraceDecoder * decoder = new chip::trace::TraceDecoder();
    decoder->SetOptions(options);
    chip::trace::AddTraceStream(decoder);
}
```

### TraceDecoderOptions

Fine-grained control over what gets decoded:

```cpp
struct TraceDecoderOptions
{
    // Protocol-level filters
    bool mEnableProtocolSecureChannel = true;
    bool mEnableProtocolInteractionModel = true;
    bool mEnableProtocolBDX = true;
    bool mEnableProtocolUserDirectedCommissioning = true;
    bool mEnableProtocolEcho = true;
    bool mEnableProtocolInteractionModelResponse = true;
    
    // Message-level filters
    bool mEnableMessageInitiator = true;
    bool mEnableMessageResponder = true;
    
    // Data-level filters
    bool mEnableDataEncryptedPayload = true;
};
```

### Combining with Other Options

Trace decoding can be combined with trace file output:

```bash
chip-tool interactive server --trace_file mytrace.txt --trace_decode 1
```

This both saves raw trace data to a file AND displays decoded output in real-time.

### Use Cases

The `--trace-decode` feature is primarily used by Matter protocol developers for:
- Debugging protocol interactions
- Analyzing message flow between chip-tool and Matter devices
- Verifying protocol compliance
- Identifying communication issues
- Understanding low-level device behavior during commissioning and operations

**Note:** Requires the chip library to be compiled with `CHIP_CONFIG_TRANSPORT_TRACE_ENABLED` enabled.

---

## Command Processing Flow

### 1. Message Reception

**File:** `WebSocketServer.cpp`

When a WebSocket message arrives:
```cpp
static int OnWebSocketCallback(struct lws * wsi, 
                               enum lws_callback_reasons reason,
                               void * user, void * in, size_t len)
{
    if (reason == LWS_CALLBACK_RECEIVE) {
        char msg[8192];
        memcpy(msg, in, len);
        msg[len] = '\0';
        server->OnWebSocketMessageReceived(msg);
    }
}
```

### 2. Message Routing

**File:** `InteractiveCommands.cpp`

```cpp
bool InteractiveServerCommand::OnWebSocketMessageReceived(char * msg)
{
    bool isAsyncReport = strlen(msg) == 0;
    uint16_t timeout = 0;
    
    // Check for async report (empty or numeric timeout)
    if (!isAsyncReport && strlen(msg) <= 5) {
        std::stringstream ss;
        ss << msg;
        ss >> timeout;
        if (!ss.fail()) {
            isAsyncReport = true;
        }
    }
    
    gInteractiveServerResult.Setup(isAsyncReport, timeout);
    VerifyOrReturnValue(!isAsyncReport, true);
    
    // Parse and execute command
    auto shouldStop = ParseCommand(msg, &gInteractiveServerResult.mStatus);
    
    // Send response
    mWebSocketServer.Send(gInteractiveServerResult.AsJsonString().c_str());
    gInteractiveServerResult.Reset();
    
    return shouldStop;
}
```

### 3. JSON and Base64 Decoding

**File:** `Commands.cpp`

```cpp
int Commands::RunInteractive(const char * command, ...)
{
    std::vector<std::string> arguments;
    
    // Decode JSON with base64 arguments
    VerifyOrReturnValue(DecodeArgumentsFromInteractiveMode(command, arguments), 
                        EXIT_FAILURE);
    
    // Convert to argc/argv and execute
    auto err = RunCommand(argc, argv, true, storageDirectory, advertiseOperational);
    return (err == CHIP_NO_ERROR) ? EXIT_SUCCESS : EXIT_FAILURE;
}
```

**DecodeArgumentsFromBase64EncodedJson()** performs:
1. Parses outer JSON to extract `cluster`, `command`, and `arguments` fields
2. Checks that arguments start with `"base64:"`
3. Decodes base64 data using `chip::Base64Decode()`
4. Parses decoded JSON to extract argument values
5. Looks up command in registered command sets
6. Builds argument vector in command-line format

### 4. Command Execution

**File:** `Commands.cpp`

```cpp
CHIP_ERROR Commands::RunCommand(int argc, char ** argv, ...)
{
    // Locate command set (e.g., "delay")
    auto * commandSet = GetCommandSet(clusterName);
    
    // Find specific command (e.g., "wait-for-commissionee")
    Command * command = commandSet->GetCommand(commandName);
    
    // Initialize arguments
    command->InitArguments(argc, argv);
    
    // Execute as interactive
    command->RunAsInteractive();
    
    return CHIP_NO_ERROR;
}
```

### 5. Result Collection

Results are accumulated in the global `gInteractiveServerResult` object:

```cpp
struct InteractiveServerResult
{
    std::vector<std::string> mResults;  // JSON result objects
    std::vector<LogEntry> mLogs;        // Base64-encoded logs
    int mStatus;                         // EXIT_SUCCESS or EXIT_FAILURE
    std::mutex mMutex;                   // Thread safety
    
    std::string AsJsonString();         // Format as JSON response
};
```

### 6. Response Transmission

**File:** `WebSocketServer.cpp`

```cpp
void WebSocketServer::Send(const char * msg)
{
    std::lock_guard<std::mutex> lock(gMutex);
    gMessageQueue.push_back(msg);  // Queue message
}
```

On `LWS_CALLBACK_SERVER_WRITEABLE`:
```cpp
for (auto & msg : gMessageQueue) {
    chip::Platform::ScopedMemoryBuffer<unsigned char> buffer;
    buffer.Calloc(LWS_PRE + msg.size());
    memcpy(&buffer[LWS_PRE], (void *) msg.c_str(), msg.size());
    lws_write(wsi, &buffer[LWS_PRE], msg.size(), LWS_WRITE_TEXT);
}
gMessageQueue.clear();
```

---

## Example: wait-for-commissionee Command

### Input Message

```json
{
  "cluster": "delay",
  "command": "wait-for-commissionee",
  "arguments": "base64:eyAibm9kZUlkIjoiMzA1NDE0OTQ1IiB9"
}
```

### Decoded Arguments

The base64 string `eyAibm9kZUlkIjoiMzA1NDE0OTQ1IiB9` decodes to:
```json
{
  "nodeId": "305414945"
}
```

### Command Registration

**File:** `commands/delay/Commands.h`

```cpp
void registerCommandsDelay(Commands & commands, CredentialIssuerCommands * credsIssuerConfig)
{
    const char * clusterName = "Delay";
    commands_list clusterCommands = {
        make_unique<SleepCommand>(credsIssuerConfig),
        make_unique<WaitForCommissioneeCommand>(credsIssuerConfig),
    };
    commands.RegisterCommandSet(clusterName, clusterCommands, 
        "Commands for waiting for something to happen.");
}
```

### Command Implementation

**File:** `commands/delay/WaitForCommissioneeCommand.cpp`

```cpp
class WaitForCommissioneeCommand : public CHIPCommand
{
public:
    WaitForCommissioneeCommand(CredentialIssuerCommands * credIssuerCommands) : 
        CHIPCommand("wait-for-commissionee", credIssuerCommands),
        mOnDeviceConnectedCallback(OnDeviceConnectedFn, this),
        mOnDeviceConnectionFailureCallback(OnDeviceConnectionFailureFn, this)
    {
        AddArgument("nodeId", 0, UINT64_MAX, &mNodeId);
        AddArgument("expire-existing-session", 0, 1, &mExpireExistingSession);
        AddArgument("timeout", 0, UINT16_MAX, &mTimeout);
    }

    CHIP_ERROR RunCommand() override
    {
        chip::FabricIndex fabricIndex = CurrentCommissioner().GetFabricIndex();
        VerifyOrReturnError(fabricIndex != chip::kUndefinedFabricIndex, 
                           CHIP_ERROR_INCORRECT_STATE);

        if (mExpireExistingSession.ValueOr(true))
        {
            CurrentCommissioner().SessionMgr()->ExpireAllSessions(
                chip::ScopedNodeId(mNodeId, fabricIndex));
        }

        return CurrentCommissioner().GetConnectedDevice(
            mNodeId, 
            &mOnDeviceConnectedCallback, 
            &mOnDeviceConnectionFailureCallback);
    }

private:
    chip::NodeId mNodeId;
    chip::Optional<bool> mExpireExistingSession;
    chip::Optional<uint16_t> mTimeout;

    static void OnDeviceConnectedFn(void * context, 
                                   Messaging::ExchangeManager & exchangeMgr,
                                   const SessionHandle & sessionHandle)
    {
        auto * command = reinterpret_cast<WaitForCommissioneeCommand *>(context);
        command->SetCommandExitStatus(CHIP_NO_ERROR);
    }

    static void OnDeviceConnectionFailureFn(void * context, 
                                           const ScopedNodeId & peerId, 
                                           CHIP_ERROR err)
    {
        auto * command = reinterpret_cast<WaitForCommissioneeCommand *>(context);
        command->SetCommandExitStatus(err);
    }

    chip::Callback::Callback<OnDeviceConnected> mOnDeviceConnectedCallback;
    chip::Callback::Callback<OnDeviceConnectionFailure> mOnDeviceConnectionFailureCallback;
};
```

### What the Command Does

The `wait-for-commissionee` command:

1. **Validates Fabric Index**: Ensures the commissioner is on a valid fabric
2. **Expires Existing Sessions** (optional, default true):
   - Calls `SessionMgr()->ExpireAllSessions()` for the target node
   - Forces a fresh CASE (Certificate Authenticated Session Establishment)
3. **Establishes Connection**:
   - Calls `GetConnectedDevice()` with the node ID
   - Registers success and failure callbacks
4. **Waits Asynchronously**:
   - Returns immediately but callbacks fire when connection completes
   - Success: `OnDeviceConnectedFn()` sets exit status to `CHIP_NO_ERROR`
   - Failure: `OnDeviceConnectionFailureFn()` sets exit status to error code

### Command-Line Equivalent

The JSON message is equivalent to:
```bash
chip-tool delay wait-for-commissionee 305414945
```

With optional parameters:
```bash
chip-tool delay wait-for-commissionee 305414945 --expire-existing-session 0 --timeout 30000
```

### Response Examples

**Successful Connection:**
```json
{
  "results": [],
  "logs": [
    {
      "module": "chipTool",
      "category": "Info",
      "message": "RGV2aWNlIGNvbm5lY3RlZCBzdWNjZXNzZnVsbHk="
    }
  ]
}
```

**Failed Connection:**
```json
{
  "results": [
    {"error": "FAILURE"}
  ],
  "logs": [
    {
      "module": "chipTool",
      "category": "Error",
      "message": "RmFpbGVkIHRvIGVzdGFibGlzaCBDQVNFIHNlc3Npb24="
    }
  ]
}
```

### Complete Flow Diagram

```
WebSocket Client
      |
      | {"cluster":"delay","command":"wait-for-commissionee","arguments":"base64:..."}
      v
WebSocketServer.OnWebSocketCallback()
      |
      v
InteractiveServerCommand::OnWebSocketMessageReceived()
      |
      v
Commands::DecodeArgumentsFromBase64EncodedJson()
      |  - Parse JSON
      |  - Decode base64 → {"nodeId":"305414945"}
      |  - Build argv: ["delay", "wait-for-commissionee", "305414945"]
      v
Commands::RunCommand()
      |  - Lookup "delay" cluster
      |  - Find "wait-for-commissionee" command
      v
WaitForCommissioneeCommand::RunCommand()
      |  - Expire existing sessions (optional)
      |  - Call GetConnectedDevice()
      |  - Register callbacks
      v
[Async Wait for CASE Session]
      |
      v
OnDeviceConnectedFn() OR OnDeviceConnectionFailureFn()
      |  - SetCommandExitStatus()
      v
gInteractiveServerResult
      |  - Collect results and logs
      |  - Format as JSON
      v
WebSocketServer.Send()
      |  - Queue message
      |  - Send on LWS_CALLBACK_SERVER_WRITEABLE
      v
WebSocket Client (receives JSON response)
```

---

## Thread Safety

The interactive server implementation uses several thread safety mechanisms:

### Mutexes

- **`gInteractiveServerResult.mMutex`**: Protects access to results and logs
- **`gMutex`**: Protects the global message queue

### ScopedLock Pattern

```cpp
std::lock_guard<std::mutex> lock(gMutex);
```

RAII-style lock management ensures proper cleanup even on exceptions.

### Message Queue

The `gMessageQueue` is accessed from multiple threads:
- **CHIP Thread**: Adds results via `RemoteDataModelLogger::LogJSON()`
- **WebSocket Thread**: Sends queued messages via `lws_write()`

All access is protected by `gMutex`.

---

## Special Features

### Operational Advertising

Flag: `--advertise-operational` (defaults to true)

- Allows advertising operational node over DNS-SD
- Accepts incoming CASE sessions in server mode
- Configurable interface ID for specific network interfaces

### Async Timeout Support

- Empty message or numeric timeout triggers async mode
- Waits for subscription reports or events
- Automatically times out if no results within specified duration
- Uses `chip::DeviceLayer::SystemLayer().StartTimer()`

### Command History

In interactive start mode (not server), chip-tool maintains command history:
- Uses readline library
- History saved to file
- Persistent across sessions

---

## References

### Source Files

- `examples/chip-tool/commands/interactive/InteractiveCommands.h`
- `examples/chip-tool/commands/interactive/InteractiveCommands.cpp`
- `examples/chip-tool/commands/common/Commands.h`
- `examples/chip-tool/commands/common/Commands.cpp`
- `examples/chip-tool/commands/common/CHIPCommand.h`
- `examples/chip-tool/commands/common/CHIPCommand.cpp`
- `examples/chip-tool/commands/delay/WaitForCommissioneeCommand.h`
- `examples/chip-tool/commands/delay/WaitForCommissioneeCommand.cpp`
- `examples/chip-tool/commands/delay/Commands.h`
- `examples/common/websocket-server/WebSocketServer.h`
- `examples/common/websocket-server/WebSocketServer.cpp`
- `examples/common/tracing/TraceDecoder.h`
- `examples/common/tracing/TraceDecoder.cpp`

### Build Configuration

From `BUILD.gn`:
- Requires `config_use_interactive_mode` flag
- Dependencies: `libwebsockets`, `editline` (for readline support)
- Trace decoding requires `CHIP_CONFIG_TRANSPORT_TRACE_ENABLED`

---

## Conclusion

The chip-tool interactive server provides a powerful WebSocket-based interface for remotely controlling Matter device commissioning and interaction. Its architecture supports:

- Structured JSON command format with base64 argument encoding
- Asynchronous command execution with callback-based completion
- Comprehensive logging with categorization and base64 encoding
- Thread-safe message queuing and result collection
- Protocol-level tracing and decoding for debugging
- Flexible command registration and execution framework

This enables chip-tool to function as a remote-controllable Matter commissioning and interaction tool, suitable for automation, testing, and integration with higher-level control systems.
