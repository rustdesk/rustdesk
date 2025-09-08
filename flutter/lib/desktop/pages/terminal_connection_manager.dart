import 'package:flutter/foundation.dart';
import 'package:get/get.dart';
import '../../models/model.dart';

/// Manages terminal connections to ensure one FFI instance per peer
class TerminalConnectionManager {
  static final Map<String, FFI> _connections = {};
  static final Map<String, int> _connectionRefCount = {};
  
  // Track service IDs per peer
  static final Map<String, String> _serviceIds = {};

  /// Get or create an FFI instance for a peer
  static FFI getConnection({
    required String peerId,
    required String? password,
    required bool? isSharedPassword,
    required bool? forceRelay,
    required String? connToken,
  }) {
    final existingFfi = _connections[peerId];
    if (existingFfi != null && !existingFfi.closed) {
      // Increment reference count
      _connectionRefCount[peerId] = (_connectionRefCount[peerId] ?? 0) + 1;
      debugPrint('[TerminalConnectionManager] Reusing existing connection for peer $peerId. Reference count: ${_connectionRefCount[peerId]}');
      return existingFfi;
    }

    // Create new FFI instance for first terminal
    debugPrint('[TerminalConnectionManager] Creating new terminal connection for peer $peerId');
    final ffi = FFI(null);
    ffi.start(
      peerId,
      password: password,
      isSharedPassword: isSharedPassword,
      forceRelay: forceRelay,
      connToken: connToken,
      isTerminal: true,
    );
    
    _connections[peerId] = ffi;
    _connectionRefCount[peerId] = 1;
    
    // Register the FFI instance with Get for dependency injection
    Get.put<FFI>(ffi, tag: 'terminal_$peerId');
    
    debugPrint('[TerminalConnectionManager] New connection created. Total connections: ${_connections.length}');
    return ffi;
  }

  /// Release a connection reference
  static void releaseConnection(String peerId) {
    final refCount = _connectionRefCount[peerId] ?? 0;
    debugPrint('[TerminalConnectionManager] Releasing connection for peer $peerId. Current ref count: $refCount');
    
    if (refCount <= 1) {
      // Last reference, close the connection
      final ffi = _connections[peerId];
      if (ffi != null) {
        debugPrint('[TerminalConnectionManager] Closing connection for peer $peerId (last reference)');
        ffi.close();
        _connections.remove(peerId);
        _connectionRefCount.remove(peerId);
        Get.delete<FFI>(tag: 'terminal_$peerId');
      }
    } else {
      // Decrement reference count
      _connectionRefCount[peerId] = refCount - 1;
      debugPrint('[TerminalConnectionManager] Connection still in use. New ref count: ${_connectionRefCount[peerId]}');
    }
  }

  /// Check if a connection exists for a peer
  static bool hasConnection(String peerId) {
    final ffi = _connections[peerId];
    return ffi != null && !ffi.closed;
  }
  
  /// Get existing connection without creating new one
  static FFI? getExistingConnection(String peerId) {
    return _connections[peerId];
  }

  /// Get connection count for debugging
  static int getConnectionCount() => _connections.length;
  
  /// Get terminal count for a peer
  static int getTerminalCount(String peerId) => _connectionRefCount[peerId] ?? 0;
  
  /// Get service ID for a peer
  static String? getServiceId(String peerId) => _serviceIds[peerId];
  
  /// Set service ID for a peer
  static void setServiceId(String peerId, String serviceId) {
    _serviceIds[peerId] = serviceId;
    debugPrint('[TerminalConnectionManager] Service ID for $peerId: $serviceId');
  }
}