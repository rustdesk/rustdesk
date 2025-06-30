# RustDesk Terminal Service Implementation

## Overview

The RustDesk terminal service provides remote terminal/shell access with support for multiple concurrent terminal sessions per connection. It features persistence support, allowing terminal sessions to survive connection drops and be resumed later.

## Architecture

### Client-Side (Flutter)

#### Terminal Connection Management
- **TerminalConnectionManager** (`flutter/lib/desktop/pages/terminal_connection_manager.dart`)
  - Manages one FFI instance per peer (shared across all terminal tabs)
  - Tracks persistence settings per peer
  - Handles connection reference counting

#### Terminal Models
- **TerminalModel** (`flutter/lib/models/terminal_model.dart`)
  - One instance per terminal tab
  - Handles terminal I/O and display using xterm package
  - Manages terminal state (opened, size, buffer)

#### UI Components
- **TerminalTabPage** (`flutter/lib/desktop/pages/terminal_tab_page.dart`)
  - Manages multiple terminal tabs
  - Right-click menu for persistence toggle
  - Keyboard shortcuts (Cmd/Ctrl+Shift+T for new terminal)

### Server-Side (Rust)

#### Terminal Service Structure
```rust
TerminalService {
    conn_id: i32,
    service_id: String,  // "tmp_{uuid}" or "persist_{uuid}"
    persist: bool,
}

PersistentTerminalService {
    service_id: String,
    sessions: HashMap<i32, TerminalSession>,  // terminal_id -> session
    next_terminal_id: i32,
    created_at: Instant,
    last_activity: Instant,
}

TerminalSession {
    terminal_id: i32,
    pty_pair: PtyPair,
    child: Box<dyn Child>,
    writer: Box<dyn Write>,
    reader: Box<dyn Read>,
    output_buffer: OutputBuffer,  // For reconnection
    rows: u16,
    cols: u16,
}
```

## Message Protocol

### Client → Server Messages

1. **Open Terminal**
```protobuf
TerminalAction {
    open: OpenTerminal {
        terminal_id: i32,
        rows: u32,
        cols: u32,
    }
}
```

2. **Send Input**
```protobuf
TerminalAction {
    data: TerminalData {
        terminal_id: i32,
        data: bytes,
    }
}
```

3. **Resize Terminal**
```protobuf
TerminalAction {
    resize: ResizeTerminal {
        terminal_id: i32,
        rows: u32,
        cols: u32,
    }
}
```

4. **Close Terminal**
```protobuf
TerminalAction {
    close: CloseTerminal {
        terminal_id: i32,
        force: bool,
    }
}
```

### Server → Client Messages

1. **Terminal Opened**
```protobuf
TerminalResponse {
    opened: TerminalOpened {
        terminal_id: i32,
        success: bool,
        message: string,
        pid: u32,
    }
}
```

2. **Terminal Output**
```protobuf
TerminalResponse {
    data: TerminalData {
        terminal_id: i32,
        data: bytes,  // Base64 encoded in Flutter
    }
}
```

3. **Terminal Closed**
```protobuf
TerminalResponse {
    closed: TerminalClosed {
        terminal_id: i32,
        exit_code: i32,
    }
}
```

## Persistence Design

### Service ID Convention
- **Temporary**: `"tmp_{uuid}"` - Cleaned up after idle timeout
- **Persistent**: `"persist_{uuid}"` - Survives disconnections

### Persistence Flow
1. User right-clicks terminal tab → "Enable terminal persistence"
2. Client stores persistence preference in `TerminalConnectionManager`
3. New terminals created with appropriate service ID prefix
4. Service ID saved for future reconnection (TODO: implement storage)

### Cleanup Rules
- **Temporary services (`tmp_`)**:
  - Removed after 1 hour idle time
  - Immediately removed when service loop exits
  
- **Persistent services**:
  - Removed after 2 hours idle time IF empty
  - Survive connection drops
  - Can be reconnected using saved service ID

### Cleanup Implementation

#### 1. **Automatic Background Cleanup**
```rust
// Runs every 5 minutes
fn ensure_cleanup_task() {
    tokio::spawn(async {
        let mut interval = tokio::time::interval(Duration::from_secs(300));
        loop {
            interval.tick().await;
            cleanup_inactive_services();
        }
    });
}
```

#### 2. **Cleanup Logic**
```rust
fn cleanup_inactive_services() {
    let now = Instant::now();
    
    for (service_id, service) in services.iter() {
        // Temporary services: clean up after 1 hour idle
        if service_id.starts_with("tmp_") && 
           now.duration_since(svc.last_activity) > SERVICE_IDLE_TIMEOUT {
            to_remove.push(service_id);
        }
        // Persistent services: clean up after 2 hours IF empty
        else if !service_id.starts_with("tmp_") && 
                svc.sessions.is_empty() && 
                now.duration_since(svc.last_activity) > SERVICE_IDLE_TIMEOUT * 2 {
            to_remove.push(service_id);
        }
    }
}
```

#### 3. **Service Loop Exit Cleanup**
```rust
fn run(sp: EmptyExtraFieldService, _conn_id: i32, service_id: String) {
    // Service loop
    while sp.ok() {
        // Read and send terminal outputs...
    }
    
    // Clean up temporary services immediately on exit
    if service_id.starts_with("tmp_") {
        remove_service(&service_id);
    }
}
```

#### 4. **Session Cleanup Within Service**
When a terminal is closed:
- PTY process is terminated
- Terminal session removed from service's HashMap
- Resources (file descriptors, buffers) are freed
- Service continues running for other terminals

#### 5. **Connection Drop Behavior**
```rust
impl Drop for Connection {
    fn drop(&mut self) {
        if self.terminal {
            // Unsubscribe from terminal service
            server.subscribe(&service_name, self.inner.clone(), false);
        }
    }
}
```
- Connection unsubscribes from service
- Service loop continues if other subscribers exist
- If no subscribers remain, `sp.ok()` returns false → service loop exits

#### 6. **Activity Tracking**
`last_activity` is updated when:
- New terminal opened
- Input sent to terminal
- Terminal resized
- Output read from terminal
- Any terminal operation occurs

#### 7. **Two-Phase Cleanup Process**
```rust
// Collect services to remove (while holding lock)
let mut to_remove = Vec::new();
for (id, service) in services.iter() {
    if should_remove(service) {
        to_remove.push(id);
    }
}

// Remove services (after releasing lock)
drop(services);
for id in to_remove {
    remove_service(&id);
}
```
This prevents deadlock when removing services.

## Key Features

### Multiple Terminals per Connection
- Single FFI connection shared by all terminal tabs
- Each terminal has unique ID within the service
- Independent PTY sessions per terminal

### Output Buffering
- Last 1MB of output buffered per terminal
- Allows showing recent history on reconnection
- Ring buffer with line-based storage

### Cross-Platform Support
- **Unix/Linux/macOS**: Uses default shell from `$SHELL` or `/bin/bash`
- **Windows**: Uses `%COMSPEC%` or `cmd.exe`
- PTY implementation via `portable_pty` crate

### Non-Blocking I/O
- PTY readers set to non-blocking mode (Unix)
- Output polled at ~33fps for responsive display
- Prevents blocking when no data available

## Current Limitations

1. **Service ID Storage**: Client doesn't persist service IDs yet
2. **Reconnection UI**: No UI to recover previous sessions
3. **Authentication**: No per-service authentication for reconnection
4. **Resource Limits**: No configurable limits on terminals per service

## Future Enhancements

1. **Proper Reconnection Flow**:
   - Store service IDs in peer config
   - UI to list and recover previous sessions
   - Show buffered output on reconnection

2. **Security**:
   - Authentication token for service recovery
   - Encryption of buffered output
   - Access control per terminal

3. **Advanced Features**:
   - Terminal sharing between users
   - Session recording/playback
   - File transfer via terminal
   - Custom shell/command configuration

## Code Locations

- **Server Implementation**: `src/server/terminal_service.rs`
- **Connection Handler**: `src/server/connection.rs` (handle_terminal_action)
- **Client Interface**: `src/ui_session_interface.rs` (terminal methods)
- **Flutter FFI**: `src/flutter_ffi.rs` (session_open_terminal, etc.)
- **Flutter Models**: `flutter/lib/models/terminal_model.dart`
- **Flutter UI**: `flutter/lib/desktop/pages/terminal_*.dart`

## Usage

1. **Start Terminal Session**:
   - Click terminal icon or use Ctrl/Cmd+Shift+T
   - Terminal opens with default shell

2. **Enable Persistence**:
   - Right-click any terminal tab
   - Select "Enable terminal persistence"
   - All terminals for that peer become persistent

3. **Multiple Terminals**:
   - Click "+" button or Ctrl/Cmd+Shift+T
   - Each terminal is independent

4. **Reconnection** (TODO):
   - Connect to same peer
   - Previous terminals automatically restored
   - Recent output displayed

## Implementation Issues & TODOs

### Critical Missing Features

1. **Service ID Storage & Recovery**
   - Need to store service_id in peer config when persistence enabled
   - Pass service_id in LoginRequest for reconnection
   - Handle service_id in server login flow
   - Return terminal list in LoginResponse

2. **Protocol Extensions Needed**
   ```protobuf
   // In LoginRequest
   message Terminal {
       string service_id = 1;  // For reconnection
       bool persistent = 2;    // Request persistence
   }
   
   // In LoginResponse
   message TerminalServiceInfo {
       string service_id = 1;
       repeated TerminalSessionInfo sessions = 2;
   }
   ```

3. **Terminal Recovery Flow**
   - Add RecoverTerminal action to restore specific terminal
   - Send buffered output on reconnection
   - Handle terminal size on recovery
   - UI to show available terminals

### Current Design Issues

1. **Service Pattern Mismatch**
   - Terminal service forced into broadcast service pattern
   - Should be direct connection resource, not shared service
   - Complex routing through service registry unnecessary

2. **Global State Management**
   - TERMINAL_SERVICES static HashMap may cause issues
   - No proper service discovery mechanism
   - Cleanup task is global, not per-connection

3. **Resource Limits Missing**
   - No limit on terminals per service
   - No limit on buffer size per terminal
   - No limit on total services
   - Could lead to resource exhaustion

4. **Security Concerns**
   - No authentication for service recovery
   - Service IDs are predictable (just UUID)
   - No encryption of buffered terminal output
   - No access control between users

### Performance Optimizations Needed

1. **Output Reading**
   - Currently polls at 33fps regardless of activity
   - Should use event-driven I/O (epoll/kqueue)
   - Batch small outputs to reduce messages

2. **Buffer Management**
   - Ring buffer could be more efficient
   - Consider compression for stored output
   - Implement smart truncation (keep last N complete lines)

3. **Message Overhead**
   - Each output chunk creates new protobuf message
   - Could batch multiple terminal outputs
   - Consider streaming protocol for continuous output

### Platform-Specific Issues

1. **Windows**
   - ConPTY support needs testing
   - Non-blocking I/O handled differently
   - Shell detection could be improved

2. **Mobile (Android/iOS)**
   - Terminal feature disabled by conditional compilation
   - Need to evaluate mobile terminal support
   - Touch keyboard integration needed

### Testing Requirements

1. **Unit Tests Needed**
   - Terminal service lifecycle
   - Cleanup logic edge cases
   - Buffer management
   - Message serialization

2. **Integration Tests**
   - Multi-terminal scenarios
   - Reconnection flows
   - Cleanup timing
   - Resource limits

3. **Stress Tests**
   - Many terminals per connection
   - Large output volumes
   - Rapid connect/disconnect
   - Long-running sessions

### Alternative Designs to Consider

1. **Direct Terminal Management**
   ```rust
   // In Connection struct
   terminals: HashMap<i32, TerminalSession>,
   
   // No service pattern, direct management
   async fn handle_terminal_action(&mut self, action) {
       match action {
           Open => self.open_terminal(),
           Data => self.terminal_input(),
           // etc
       }
   }
   ```

2. **Actor-Based Design**
   - Each terminal as an actor
   - Message passing for I/O
   - Better isolation and error handling

3. **Session Manager Service**
   - One global terminal manager
   - Connections request terminals from manager
   - Cleaner separation of concerns

### Documentation Gaps

1. **API Documentation**
   - Document all public methods
   - Add examples for common operations
   - Document error conditions

2. **Configuration**
   - Document all timeouts and limits
   - How to configure shell/terminal
   - Platform-specific settings

3. **Troubleshooting Guide**
   - Common issues and solutions
   - Debug logging interpretation
   - Performance tuning

### Future Feature Ideas

1. **Advanced Terminal Features**
   - Terminal sharing (multiple users, one terminal)
   - Session recording and playback
   - File transfer through terminal (zmodem)
   - Custom color schemes
   - Font configuration

2. **Integration Features**
   - SSH key forwarding
   - Environment variable injection
   - Working directory synchronization
   - Shell integration (prompt markers, etc)

3. **Management Features**
   - Terminal session monitoring
   - Usage statistics
   - Audit logging
   - Rate limiting

### Refactoring Suggestions

1. **Separate Concerns**
   - Split terminal_service.rs into multiple files
   - Separate PTY management from service logic
   - Extract buffer management to own module

2. **Improve Error Handling**
   - Use proper error types, not strings
   - Add error recovery mechanisms
   - Better error reporting to client

3. **Configuration Management**
   - Make timeouts configurable
   - Add feature flags for experimental features
   - Environment-based configuration