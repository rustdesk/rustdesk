import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:provider/provider.dart';
// to-do: do not depend on desktop
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
      child: Consumer<PluginModel>(
        builder: (context, pluginModel, child) {
          return Column(
            children: pluginModel.uiList.map((ui) => _buildItem(ui)).toList(),
          );
        },
      ),
    );
  }

  Widget _buildItem(UiType ui) {
    late Widget child;
    switch (ui.runtimeType) {
      case UiButton:
        child = _buildMenuButton(ui as UiButton);
        break;
      case UiCheckbox:
        child = _buildCheckboxMenuButton(ui as UiCheckbox);
        break;
      default:
        child = Container();
    }
    // to-do: add plugin icon and tooltip
    return child;
  }

  Uint8List _makeEvent(
    String key, {
    bool? v,
  }) {
    final event = MsgFromUi(
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
      onPressed: () => bind.pluginEvent(
        id: pluginId,
        peer: peerId,
        event: _makeEvent(ui.key),
      ),
      trailingIcon: Icon(
          IconData(int.parse(ui.icon, radix: 16), fontFamily: 'MaterialIcons')),
      // to-do: RustDesk translate or plugin translate ?
      child: Text(ui.text),
      ffi: ffi,
    );
  }

  String? getOption(OptionModel model, String key) {
    var v = model.value;
    if (v == null) {
      if (peerId.isEmpty) {
        v = bind.pluginGetLocalOption(id: pluginId, key: key);
      } else {
        v = bind.pluginGetSessionOption(id: pluginId, peer: peerId, key: key);
      }
    }
    return v;
  }

  Widget _buildCheckboxMenuButton(UiCheckbox ui) {
    getChild(OptionModel model) {
      final v = getOption(model, ui.key);
      if (v == null) {
        // session or plugin not found
        return Container();
      }
      return CkbMenuButton(
        value: ConfigItem.isTrue(v),
        onChanged: (v) {
          if (v != null) {
            bind.pluginEvent(
              id: pluginId,
              peer: peerId,
              event: _makeEvent(ui.key, v: v),
            );
          }
        },
        // to-do: RustDesk translate or plugin translate ?
        child: Text(ui.text),
        ffi: ffi,
      );
    }

    final optionModel = addOptionModel(location, pluginId, peerId, ui.key);
    return ChangeNotifierProvider.value(
      value: optionModel,
      child: Consumer<OptionModel>(
        builder: (context, model, child) {
          return getChild(model);
        },
      ),
    );
  }
}

void handleReloading(Map<String, dynamic> evt, String peer) {
  if (evt['id'] == null || evt['location'] == null) {
    return;
  }
  addLocationUi(evt['location']!, evt['id']!, UiType.fromJson(evt));
}

void handleOption(Map<String, dynamic> evt, String peer) {
  updateOption(
      evt['location'], evt['id'], evt['peer'] ?? '', evt['key'], evt['value']);
}
