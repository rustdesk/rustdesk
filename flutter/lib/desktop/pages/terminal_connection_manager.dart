import 'package:flutter/foundation.dart';
import 'package:get/get.dart';
import 'package:uuid/uuid.dart';
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
    String? connToken,
  }) {
    final existingFfi = _connections[peerId];
    if (existingFfi != null && !existingFfi.closed) {
      // Increment reference count
      _connectionRefCount[peerId] = (_connectionRefCount[peerId] ?? 0) + 1;
      debugPrint('[TerminalConnectionManager] Reusing existing connection for peer $peerId. Reference count: ${_connectionRefCount[peerId]}');
      return existingFfi;
    }

    // Create new FFI instance for first terminal.
    // IMPORTANT: pass a fresh SessionID. On mobile FFI(null) reuses a shared
    // constant SessionID, which would collide with the active video session's
    // FFI — the native side then injects a "close" into the video stream and
    // never spawns the terminal's io_loop, so the terminal hangs on
    // "Connecting…". A unique id makes the terminal a distinct session
    // (desktop already does this).
    debugPrint('[TerminalConnectionManager] Creating new terminal connection for peer $peerId');
    final ffi = FFI(const Uuid().v4obj());
    // Track the connection BEFORE start() so a throw can't leave an orphaned,
    // half-started native session that nothing ever closes.
    _connections[peerId] = ffi;
    _connectionRefCount[peerId] = 1;
    Get.put<FFI>(ffi, tag: 'terminal_$peerId');
    try {
      ffi.start(
        peerId,
        password: password,
        isSharedPassword: isSharedPassword,
        forceRelay: forceRelay,
        connToken: connToken,
        isTerminal: true,
      );
    } catch (e) {
      debugPrint('[TerminalConnectionManager] start failed for $peerId: $e');
      _connections.remove(peerId);
      _connectionRefCount.remove(peerId);
      Get.delete<FFI>(tag: 'terminal_$peerId', force: true);
      ffi.close();
      rethrow;
    }

    debugPrint('[TerminalConnectionManager] New connection created. Total connections: ${_connections.length}');
    return ffi;
  }

  /// Release a connection reference
  static void releaseConnection(String peerId) {
    final refCount = _connectionRefCount[peerId] ?? 0;
    debugPrint('[TerminalConnectionManager] Releasing connection for peer $peerId. Current ref count: $refCount');
    
    if (refCount <= 1) {
      // Last reference: tear everything down. Clear all bookkeeping even if the
      // FFI is already gone, so a desync can't leave a stale refcount/service id
      // behind that poisons later getTerminalCount()/hasConnection() checks.
      debugPrint('[TerminalConnectionManager] Closing connection for peer $peerId (last reference)');
      _connections.remove(peerId)?.close();
      _connectionRefCount.remove(peerId);
      _serviceIds.remove(peerId);
      Get.delete<FFI>(tag: 'terminal_$peerId', force: true);
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