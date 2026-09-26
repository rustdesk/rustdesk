import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/desktop/widgets/tabbar_widget.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/models/usbip_model.dart';
import 'package:get/get.dart';
import 'package:provider/provider.dart';

class UsbipPage extends StatefulWidget {
  UsbipPage({
    Key? key,
    required this.id,
    required this.password,
    required this.tabController,
    required this.isSharedPassword,
    this.forceRelay,
    this.connToken,
  }) : super(key: key);
  final String id;
  final String? password;
  final DesktopTabController tabController;
  final bool? forceRelay;
  final bool? isSharedPassword;
  final String? connToken;

  @override
  State<UsbipPage> createState() => _UsbipPageState();
}

class _UsbipPageState extends State<UsbipPage> {
  late final FFI _ffi;

  @override
  void initState() {
    super.initState();
    _ffi = FFI(null);
    _ffi.start(widget.id,
        isRemoteUsb: true,
        password: widget.password,
        isSharedPassword: widget.isSharedPassword,
        forceRelay: widget.forceRelay,
        connToken: widget.connToken);
    Get.put<FFI>(_ffi, tag: 'usbip_${widget.id}');
    WidgetsBinding.instance.addPostFrameCallback((_) {
      widget.tabController.onSelected?.call(widget.id);
    });
  }

  @override
  void dispose() {
    // Closing the session detaches and unshares exactly what this session
    // attached or shared itself (`UsbClientState::close` on the Rust side,
    // `UsbipMux::close_all` on the peer); `UsbDeviceInfo.shared` is
    // machine-wide and may belong to the CLI or another session.
    _ffi.close();
    _ffi.dialogManager.dismissAll();
    Get.delete<FFI>(tag: 'usbip_${widget.id}');
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return ChangeNotifierProvider.value(
      value: _ffi.usbipModel,
      child: Scaffold(
        backgroundColor: Theme.of(context).scaffoldBackgroundColor,
        body: Consumer<UsbipModel>(
          builder: (context, model, _) => _buildBody(context, model),
        ),
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
                title: translate('My local devices'),
                loading: false,
                onRefresh: model.requestLocalDevices,
              ),
              if (model.localDevices.isEmpty)
                Padding(
                  padding: const EdgeInsets.symmetric(
                      horizontal: 12, vertical: 8),
                  child: Text(translate('No USB devices')),
                )
              else
                ...model.localDevices.map(
                    (device) => _buildLocalDeviceRow(context, model, device)),
              const Divider(height: 24),
              _buildSectionHeader(
                context,
                title: translate('Peer\'s devices'),
                loading: model.loading,
                onRefresh: model.loading ? null : model.requestDevices,
              ),
              if (model.devices.isEmpty)
                Padding(
                  padding: const EdgeInsets.symmetric(
                      horizontal: 12, vertical: 8),
                  child: Text(model.loading
                      ? translate('Loading...')
                      : translate('No USB devices')),
                )
              else
                ...model.devices
                    .map((device) => _buildDeviceRow(context, model, device)),
            ],
          ),
        ),
      ],
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

  Widget _buildLocalDeviceRow(
      BuildContext context, UsbipModel model, UsbDeviceInfo device) {
    final pending = model.localPendingBusIds.contains(device.busId);
    final pushed = device.pushed;
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
                  '${device.busId}  ${device.vendor}',
                  style: TextStyle(
                      fontSize: 12, color: Theme.of(context).hintColor),
                ),
              ],
            ),
          ),
          ElevatedButton(
            onPressed: pending
                ? null
                : () => model.togglePush(device.busId, !pushed),
            child: Text(pushed ? translate('Unpush') : translate('Push')),
          ),
        ],
      ),
    );
  }

  Widget _buildDeviceRow(
      BuildContext context, UsbipModel model, UsbDeviceInfo device) {
    final pending = model.pendingBusIds.contains(device.busId);
    final attachedPort = model.attachedPorts[device.busId];
    final attached = attachedPort != null;
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
                  '${device.busId}  ${device.vendor}',
                  style: TextStyle(
                      fontSize: 12, color: Theme.of(context).hintColor),
                ),
              ],
            ),
          ),
          ElevatedButton(
            onPressed: pending
                ? null
                : () => model.toggleAttach(device.busId, !attached),
            child: Text(attached ? translate('Detach') : translate('Attach')),
          ),
        ],
      ),
    );
  }
}
