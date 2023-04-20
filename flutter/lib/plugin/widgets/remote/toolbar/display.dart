import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:provider/provider.dart';
import 'package:flutter_hbb/desktop/widgets/remote_toolbar.dart';
import 'package:flutter_hbb/models/platform_model.dart';

import '../../../desc.dart';
import '../../../model.dart';
import '../../../common.dart';

class Display extends StatelessWidget {
  final PluginId pluginId;
  final String peerId;
  final FFI ffi;
  final String location;
  final LocationModel locationModel;

  Display({
    Key? key,
    required this.pluginId,
    required this.peerId,
    required this.ffi,
    required this.location,
    required this.locationModel,
  }) : super(key: key);

  bool get isEmpty => locationModel.isEmpty;

  @override
  Widget build(BuildContext context) {
    return ChangeNotifierProvider.value(
      value: locationModel,
      child: Consumer<LocationModel>(builder: (context, model, child) {
        return Column(
          children: locationModel.uiList.map((ui) => _buildItem(ui)).toList(),
        );
      }),
    );
  }

  Widget _buildItem(UiType ui) {
    switch (ui.runtimeType) {
      case UiButton:
        return _buildMenuButton(ui as UiButton);
      case UiCheckbox:
        return _buildCheckboxMenuButton(ui as UiCheckbox);
      default:
        return Container();
    }
  }

  Uint8List _makeEvent(
    String localPeerId,
    String key, {
    bool? v,
  }) {
    final event = MsgFromUi(
      remotePeerId: peerId,
      localPeerId: localPeerId,
      id: pluginId,
      name: getDesc(pluginId)?.name ?? '',
      location: location,
      key: key,
      value:
          v != null ? (v ? ConfigItem.trueValue : ConfigItem.falseValue) : '',
      action: '',
    );
    return Uint8List.fromList(event.toString().codeUnits);
  }

  Widget _buildMenuButton(UiButton ui) {
    return MenuButton(
      onPressed: () {
        () async {
          final localPeerId = await bind.mainGetMyId();
          bind.pluginEvent(
            id: pluginId,
            event: _makeEvent(localPeerId, ui.key),
          );
        }();
      },
      // to-do: rustdesk translate or plugin translate ?
      child: Text(ui.text),
      ffi: ffi,
    );
  }

  Widget _buildCheckboxMenuButton(UiCheckbox ui) {
    final v =
        bind.pluginGetSessionOption(id: pluginId, peer: peerId, key: ui.key);
    return CkbMenuButton(
      value: ConfigItem.isTrue(v),
      onChanged: (v) {
        if (v != null) {
          () async {
            final localPeerId = await bind.mainGetMyId();
            bind.pluginEvent(
              id: pluginId,
              event: _makeEvent(localPeerId, ui.key, v: v),
            );
          }();
        }
      },
      // to-do: rustdesk translate or plugin translate ?
      child: Text(ui.text),
      ffi: ffi,
    );
  }
}
