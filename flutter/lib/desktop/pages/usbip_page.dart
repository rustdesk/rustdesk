import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
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
        Padding(
          padding: const EdgeInsets.all(12),
          child: Text(
            translate('USB forwarding'),
            style: const TextStyle(fontSize: 16, fontWeight: FontWeight.bold),
          ),
        ),
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
/// as a dialog in that session's window.
void showUsbForwardingDialog(FFI ffi) {
  ffi.dialogManager.show((setState, close, context) {
    return CustomAlertDialog(
      title: null,
      content: SizedBox(width: 480, height: 420, child: UsbipPage(ffi: ffi)),
      actions: [dialogButton('Close', onPressed: close)],
      onCancel: close,
    );
  }, tag: 'usb-forwarding');
}
