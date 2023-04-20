import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:provider/provider.dart';
import 'package:flutter_hbb/desktop/widgets/remote_toolbar.dart';
import 'package:flutter_hbb/models/platform_model.dart';

import './desc.dart';
import './model.dart';
import './common.dart';

class LocationItem extends StatelessWidget {
  final String peerId;
  final FFI ffi;
  final String location;
  final LocationModel locationModel;

  LocationItem({
    Key? key,
    required this.peerId,
    required this.ffi,
    required this.location,
    required this.locationModel,
  }) : super(key: key);

  bool get isEmpty => locationModel.isEmpty;

  static LocationItem createLocationItem(
      String peerId, FFI ffi, String location) {
    final model = addLocation(location);
    return LocationItem(
      peerId: peerId,
      ffi: ffi,
      location: location,
      locationModel: model,
    );
  }

  @override
  Widget build(BuildContext context) {
    return ChangeNotifierProvider.value(
      value: locationModel,
      child: Consumer<LocationModel>(builder: (context, model, child) {
        return Column(
          children: model.pluginModels.entries
              .map((entry) => _buildPluginItem(entry.key, entry.value))
              .toList(),
        );
      }),
    );
  }

  Widget _buildPluginItem(PluginId id, PluginModel model) => PluginItem(
        pluginId: id,
        peerId: peerId,
        ffi: ffi,
        location: location,
        pluginModel: model,
      );
}

class PluginItem extends StatelessWidget {
  final PluginId pluginId;
  final String peerId;
  final FFI ffi;
  final String location;
  final PluginModel pluginModel;

  PluginItem({
    Key? key,
    required this.pluginId,
    required this.peerId,
    required this.ffi,
    required this.location,
    required this.pluginModel,
  }) : super(key: key);

  bool get isEmpty => pluginModel.isEmpty;

  @override
  Widget build(BuildContext context) {
    return ChangeNotifierProvider.value(
      value: pluginModel,
      child: Consumer<PluginModel>(builder: (context, model, child) {
        return Column(
          children: model.uiList.map((ui) => _buildItem(ui)).toList(),
        );
      }),
    );
  }

  // to-do: add plugin icon and tooltip
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
      trailingIcon: Icon(
          IconData(int.parse(ui.icon, radix: 16), fontFamily: 'MaterialIcons')),
      // to-do: RustDesk translate or plugin translate ?
      child: Text(ui.text),
      ffi: ffi,
    );
  }

  Widget _buildCheckboxMenuButton(UiCheckbox ui) {
    final v =
        bind.pluginGetSessionOption(id: pluginId, peer: peerId, key: ui.key);
    if (v == null) {
      // session or plugin not found
      return Container();
    }
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

void handleReloading(Map<String, dynamic> evt, String peer) {
  if (evt['id'] == null || evt['location'] == null) {
    return;
  }
  final ui = UiType.fromJson(evt);
  addLocationUi(evt['location']!, evt['id']!, ui);
}
