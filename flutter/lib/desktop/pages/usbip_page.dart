import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/common/widgets/overlay.dart' as overlay;
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/models/usbip_model.dart';
import 'package:provider/provider.dart';

class UsbipPage extends StatefulWidget {
  const UsbipPage({Key? key, required this.ffi}) : super(key: key);
  final FFI ffi;

  @override
  State<UsbipPage> createState() => _UsbipPageState();
}

class _UsbipPageState extends State<UsbipPage> {
  @override
  void initState() {
    super.initState();
    widget.ffi.usbipModel.requestDevices();
    widget.ffi.usbipModel.requestLocalDevices();
  }

  @override
  Widget build(BuildContext context) {
    return ChangeNotifierProvider.value(
      value: widget.ffi.usbipModel,
      child: Consumer<UsbipModel>(
        builder: (context, model, _) => _buildBody(context, model),
      ),
    );
  }

  Widget _buildBody(BuildContext context, UsbipModel model) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (model.lastError != null)
          Container(
            width: double.infinity,
            color: Colors.red.withOpacity(0.15),
            padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
            child: Row(
              children: [
                Expanded(
                    child: Text(model.lastError!,
                        style: const TextStyle(color: Colors.red))),
                IconButton(
                  icon: const Icon(Icons.close, size: 16),
                  onPressed: () => setState(() => model.lastError = null),
                ),
              ],
            ),
          ),
        Expanded(
          child: ListView(
            children: [
              _buildSectionHeader(
                context,
                title: translate('Use my devices on the remote computer'),
                loading: false,
                onRefresh: model.requestLocalDevices,
              ),
              if (model.localDevices.isEmpty)
                _buildEmpty(translate('No USB devices'))
              else
                ...model.localDevices.map((device) => _buildDeviceRow(
                      context,
                      device,
                      forwarding: device.pushed,
                      pending: model.localPendingBusIds.contains(device.busId),
                      pendingHint: translate(
                          'Confirm on this computer and on the remote computer if asked'),
                      error: model.localErrors[device.busId],
                      onToggle: (start) => model.togglePush(device.busId, start),
                    )),
              const Divider(height: 24),
              _buildSectionHeader(
                context,
                title: translate('Use remote devices on this computer'),
                loading: model.loading,
                onRefresh: model.loading ? null : model.requestDevices,
              ),
              if (model.devices.isEmpty)
                _buildEmpty(model.loading
                    ? translate('Loading...')
                    : translate('No USB devices'))
              else
                ...model.devices.map((device) => _buildDeviceRow(
                      context,
                      device,
                      forwarding: model.attachedPorts.containsKey(device.busId),
                      pending: model.pendingBusIds.contains(device.busId),
                      pendingHint:
                          translate('Confirm on the remote computer if asked'),
                      error: model.remoteErrors[device.busId],
                      onToggle: (start) =>
                          model.toggleAttach(device.busId, start),
                    )),
            ],
          ),
        ),
      ],
    );
  }

  Widget _buildEmpty(String text) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
      child: Text(text),
    );
  }

  Widget _buildSectionHeader(
    BuildContext context, {
    required String title,
    required bool loading,
    required VoidCallback? onRefresh,
  }) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
      child: Row(
        children: [
          Text(title, style: const TextStyle(fontWeight: FontWeight.bold)),
          const Spacer(),
          if (loading)
            const SizedBox(
              width: 16,
              height: 16,
              child: CircularProgressIndicator(strokeWidth: 2),
            ),
          IconButton(
            tooltip: translate('Refresh'),
            icon: const Icon(Icons.refresh),
            onPressed: onRefresh,
          ),
        ],
      ),
    );
  }

  /// One action per device: Start shares and attaches it on the other side,
  /// Stop undoes both.
  Widget _buildDeviceRow(
    BuildContext context,
    UsbDeviceInfo device, {
    required bool forwarding,
    required bool pending,
    required String pendingHint,
    required String? error,
    required void Function(bool start) onToggle,
  }) {
    final hint = Theme.of(context).hintColor;
    final status = forwarding ? '  ·  ${translate('Forwarding')}' : '';
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
      child: Row(
        children: [
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  device.product.isNotEmpty ? device.product : device.busId,
                  style: const TextStyle(fontWeight: FontWeight.w500),
                ),
                Text(
                  '${device.busId}  ${device.vendor}$status',
                  style: TextStyle(fontSize: 12, color: hint),
                ),
                if (pending)
                  Text(pendingHint, style: TextStyle(fontSize: 12, color: hint)),
                if (error != null)
                  Text(error,
                      style: const TextStyle(fontSize: 12, color: Colors.red)),
              ],
            ),
          ),
          if (pending)
            const Padding(
              padding: EdgeInsets.only(right: 8),
              child: SizedBox(
                width: 16,
                height: 16,
                child: CircularProgressIndicator(strokeWidth: 2),
              ),
            ),
          ElevatedButton(
            onPressed: pending ? null : () => onToggle(!forwarding),
            child: Text(translate(forwarding ? 'Stop' : 'Start')),
          ),
        ],
      ),
    );
  }
}

/// USB forwarding runs over the remote session's own connection, so it opens
/// as a floating window over that session. It does not block the remote
/// screen: an elevation prompt there must stay reachable while it is open.
void toggleUsbForwardingWindow(BuildContext context, FFI ffi) {
  final open = _usbWindows.remove(ffi.sessionId);
  if (open != null && open.mounted) {
    open.remove();
    return;
  }
  final overlayState = Overlay.maybeOf(context);
  if (overlayState == null) return;
  if (!_usbWindowPositionLoaded) {
    _usbWindowPosition.load();
    _usbWindowPositionLoaded = true;
  }
  if (_usbWindowPosition.isInvalid()) {
    _usbWindowPosition.update(const Offset(80, 80));
  }
  late final OverlayEntry entry;
  void close() {
    if (entry.mounted) entry.remove();
    _usbWindows.remove(ffi.sessionId);
  }

  entry = OverlayEntry(
    builder: (context) => overlay.Draggable(
      checkScreenSize: true,
      position: _usbWindowPosition,
      width: _kUsbWindowWidth,
      height: _kUsbWindowHeight,
      builder: (context, onPanUpdate) => Material(
        elevation: 8,
        color: Theme.of(context).scaffoldBackgroundColor,
        shape: RoundedRectangleBorder(
            side: BorderSide(color: Theme.of(context).dividerColor)),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            GestureDetector(
              onPanUpdate: onPanUpdate,
              child: Container(
                height: 38,
                padding: const EdgeInsets.only(left: 12),
                child: Row(
                  children: [
                    const Icon(Icons.usb, size: 18),
                    const SizedBox(width: 8),
                    Expanded(
                      child: Text(
                        '${translate('USB forwarding')} (beta)',
                        style: const TextStyle(fontWeight: FontWeight.bold),
                      ),
                    ),
                    IconButton(
                      tooltip: translate('Close'),
                      icon: const Icon(Icons.close, size: 18),
                      onPressed: close,
                    ),
                  ],
                ),
              ),
            ),
            const Divider(height: 1),
            Expanded(child: UsbipPage(ffi: ffi)),
          ],
        ),
      ),
    ),
  );
  overlayState.insert(entry);
  _usbWindows[ffi.sessionId] = entry;
}

const double _kUsbWindowWidth = 460;
const double _kUsbWindowHeight = 440;
final Map<SessionID, OverlayEntry> _usbWindows = {};
final _usbWindowPosition =
    overlay.DraggableKeyPosition('draggablePositionUsbWindow');
bool _usbWindowPositionLoaded = false;
