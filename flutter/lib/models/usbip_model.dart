import 'package:flutter/foundation.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/models/platform_model.dart';

class UsbDeviceInfo {
  final String busId;
  final String vendor;
  final String product;
  final bool shared;

  UsbDeviceInfo.fromJson(Map<String, dynamic> json)
      : busId = json['bus_id']?.toString() ?? '',
        vendor = json['vendor']?.toString() ?? '',
        product = json['product']?.toString() ?? '',
        shared = json['shared'] == true;
}

/// Controller-side state for a RemoteUsb session: the list of devices the
/// peer is offering, and which of them this side has bound/attached.
class UsbipModel with ChangeNotifier {
  final WeakReference<FFI> parent;

  UsbipModel(this.parent);

  List<UsbDeviceInfo> devices = [];
  bool loading = false;
  String? lastError;

  // busId -> local vhci port, for devices attached from this side.
  final Map<String, int> attachedPorts = {};
  // busIds with a bind/attach/detach request in flight.
  final Set<String> pendingBusIds = {};
  // busIds whose in-flight bind request is the "share" half of a combined
  // attach (see `toggleAttach`) -- distinguishes that from a standalone
  // bind result when `handleBindResult` sees it come back.
  final Set<String> _pendingShareForAttach = {};

  // This side's own devices, offered to the peer (push direction). A local
  // device's `shared` field doubles as "pushed" here -- push/unpush always
  // share/unshare as part of the same action (see `togglePush`), so the two
  // can't drift apart the way independent share and push toggles could.
  List<UsbDeviceInfo> localDevices = [];
  // busIds with a push/unpush request in flight.
  final Set<String> localPendingBusIds = {};

  SessionID get _sessionId => parent.target!.sessionId;

  void requestDevices() {
    loading = true;
    notifyListeners();
    bind.sessionRequestUsbDevices(sessionId: _sessionId);
  }

  /// One action either way, mirroring the local-devices Push/Unpush design:
  /// attaching asks the peer to share first, then attaches once that's
  /// confirmed (`handleBindResult` completes the sequence); detaching
  /// detaches locally first, then asks the peer to unshare -- the peer
  /// retries that unshare briefly on its own if it's still momentarily
  /// "busy" from the detach, so there's nothing to sequence here for that
  /// direction.
  void toggleAttach(String busId, bool attach) {
    pendingBusIds.add(busId);
    notifyListeners();
    if (attach) {
      _pendingShareForAttach.add(busId);
      bind.sessionUsbBind(sessionId: _sessionId, busId: busId, bind: true);
    } else {
      final port = attachedPorts[busId];
      if (port != null) {
        bind.sessionUsbDetach(sessionId: _sessionId, port: port);
        attachedPorts.remove(busId);
      }
      bind.sessionUsbBind(sessionId: _sessionId, busId: busId, bind: false);
    }
  }

  void updateDeviceList(Map<String, dynamic> evt) {
    loading = false;
    final raw = evt['devices'] as List<dynamic>? ?? [];
    devices = raw
        .map((e) => UsbDeviceInfo.fromJson(e as Map<String, dynamic>))
        .toList();
    notifyListeners();
  }

  void handleBindResult(Map<String, dynamic> evt) {
    final busId = evt['bus_id']?.toString() ?? '';
    final error = evt['error']?.toString() ?? '';
    if (evt['bind'] == true && _pendingShareForAttach.remove(busId)) {
      if (error.isNotEmpty) {
        pendingBusIds.remove(busId);
        lastError = error;
        notifyListeners();
      } else {
        bind.sessionUsbAttach(sessionId: _sessionId, busId: busId);
      }
      return;
    }
    pendingBusIds.remove(busId);
    if (error.isNotEmpty) {
      lastError = error;
      notifyListeners();
    } else {
      requestDevices();
    }
  }

  void handleAttached(Map<String, dynamic> evt) {
    final busId = evt['bus_id']?.toString() ?? '';
    pendingBusIds.remove(busId);
    final success = evt['success'] == true;
    final port = evt['port'] is int ? evt['port'] as int : -1;
    if (success && port >= 0) {
      attachedPorts[busId] = port;
      requestDevices();
    } else {
      lastError = evt['message']?.toString() ?? 'Failed to attach device';
      // Roll back the share we just asked for, since the attach it was for
      // failed -- mirrors the push direction's rollback on a failed attach.
      bind.sessionUsbBind(sessionId: _sessionId, busId: busId, bind: false);
    }
    notifyListeners();
  }

  // Local devices are read straight off this machine, so there's no network
  // round trip / spinner to show for the request itself.
  void requestLocalDevices() {
    bind.sessionUsbLocalDevices(sessionId: _sessionId);
  }

  /// Push direction: one action either way. Pushing shares the device and
  /// then asks the peer to attach it; unpushing detaches it from the peer
  /// and then unshares it -- both handled together on the Rust side so
  /// there's nothing here that can leave a device shared-but-not-pushed or
  /// vice versa.
  void togglePush(String busId, bool push) {
    localPendingBusIds.add(busId);
    notifyListeners();
    if (push) {
      bind.sessionUsbPush(sessionId: _sessionId, busId: busId);
    } else {
      bind.sessionUsbUnpush(sessionId: _sessionId, busId: busId);
    }
  }

  void updateLocalDeviceList(Map<String, dynamic> evt) {
    final raw = evt['devices'] as List<dynamic>? ?? [];
    localDevices = raw
        .map((e) => UsbDeviceInfo.fromJson(e as Map<String, dynamic>))
        .toList();
    notifyListeners();
  }

  void handlePushResult(Map<String, dynamic> evt) {
    final busId = evt['bus_id']?.toString() ?? '';
    localPendingBusIds.remove(busId);
    final error = evt['error']?.toString() ?? '';
    if (error.isEmpty) {
      requestLocalDevices();
    } else {
      lastError = error;
      notifyListeners();
    }
  }
}
