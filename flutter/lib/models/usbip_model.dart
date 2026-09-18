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

  SessionID get _sessionId => parent.target!.sessionId;

  void requestDevices() {
    loading = true;
    notifyListeners();
    bind.sessionRequestUsbDevices(sessionId: _sessionId);
  }

  void toggleShare(String busId, bool share) {
    pendingBusIds.add(busId);
    notifyListeners();
    bind.sessionUsbBind(sessionId: _sessionId, busId: busId, bind: share);
  }

  void attachDevice(String busId) {
    pendingBusIds.add(busId);
    notifyListeners();
    bind.sessionUsbAttach(sessionId: _sessionId, busId: busId);
  }

  void detachDevice(String busId) {
    final port = attachedPorts[busId];
    if (port == null) return;
    bind.sessionUsbDetach(sessionId: _sessionId, port: port);
    attachedPorts.remove(busId);
    notifyListeners();
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
    pendingBusIds.remove(busId);
    final error = evt['error']?.toString() ?? '';
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
    } else {
      lastError = evt['message']?.toString() ?? 'Failed to attach device';
    }
    notifyListeners();
  }
}
