import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:provider/provider.dart';
import 'package:get/get.dart';
// to-do: do not depend on desktop
import 'package:flutter_hbb/desktop/widgets/remote_toolbar.dart';
import 'package:flutter_hbb/models/platform_model.dart';

import './desc.dart';
import './model.dart';
import './common.dart';

// dup to flutter\lib\desktop\pages\desktop_setting_page.dart
const double _kCheckBoxLeftMargin = 10;

class LocationItem extends StatelessWidget {
  final String peerId;
  final FFI ffi;
  final String location;
  final LocationModel locationModel;
  final bool isMenu;

  LocationItem({
    Key? key,
    required this.peerId,
    required this.ffi,
    required this.location,
    required this.locationModel,
    required this.isMenu,
  }) : super(key: key);

  bool get isEmpty => locationModel.isEmpty;

  static Widget createLocationItem(
      String peerId, FFI ffi, String location, bool isMenu) {
    final model = getLocationModel(location);
    return model == null
        ? Container()
        : LocationItem(
            peerId: peerId,
            ffi: ffi,
            location: location,
            locationModel: model,
            isMenu: isMenu,
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
        isMenu: isMenu,
      );
}

class PluginItem extends StatelessWidget {
  final PluginId pluginId;
  final String peerId;
  final FFI? ffi;
  final String location;
  final PluginModel pluginModel;
  final bool isMenu;

  PluginItem({
    Key? key,
    required this.pluginId,
    required this.peerId,
    this.ffi,
    required this.location,
    required this.pluginModel,
    required this.isMenu,
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
    Widget? child;
    switch (ui.runtimeType) {
      case UiButton:
        if (isMenu) {
          if (ffi != null) {
            child = _buildMenuButton(ui as UiButton, ffi!);
          }
        } else {
          child = _buildButton(ui as UiButton);
        }
        break;
      case UiCheckbox:
        if (isMenu) {
          if (ffi != null) {
            child = _buildCheckboxMenuButton(ui as UiCheckbox, ffi!);
          }
        } else {
          child = _buildCheckbox(ui as UiCheckbox);
        }
        break;
      default:
        break;
    }
    // to-do: add plugin icon and tooltip
    return child ?? Container();
  }

  Widget _buildButton(UiButton ui) {
    return TextButton(
      onPressed: () => bind.pluginEvent(
        id: pluginId,
        peer: peerId,
        event: _makeEvent(ui.key),
      ),
      child: Text(ui.text),
    );
  }

  Widget _buildCheckbox(UiCheckbox ui) {
    getChild(OptionModel model) {
      final v = _getOption(model, ui.key);
      if (v == null) {
        // session or plugin not found
        return Container();
      }

      onChanged(bool value) {
        bind.pluginEvent(
          id: pluginId,
          peer: peerId,
          event: _makeEvent(ui.key, v: value),
        );
      }

      final value = ConfigItem.isTrue(v);
      return GestureDetector(
        child: Row(
          children: [
            Checkbox(
              value: value,
              onChanged: (_) => onChanged(!value),
            ).marginOnly(right: 5),
            Expanded(
              child: Text(translate(ui.text)),
            )
          ],
        ).marginOnly(left: _kCheckBoxLeftMargin),
        onTap: () => onChanged(!value),
      );
    }

    return ChangeNotifierProvider.value(
      value: getOptionModel(location, pluginId, peerId, ui.key),
      child: Consumer<OptionModel>(
        builder: (context, model, child) => getChild(model),
      ),
    );
  }

  Widget _buildCheckboxMenuButton(UiCheckbox ui, FFI ffi) {
    getChild(OptionModel model) {
      final v = _getOption(model, ui.key);
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

    return ChangeNotifierProvider.value(
      value: getOptionModel(location, pluginId, peerId, ui.key),
      child: Consumer<OptionModel>(
        builder: (context, model, child) => getChild(model),
      ),
    );
  }

  Widget _buildMenuButton(UiButton ui, FFI ffi) {
    return MenuButton(
      onPressed: () => bind.pluginEvent(
        id: pluginId,
        peer: peerId,
        event: _makeEvent(ui.key),
      ),
      // to-do: support trailing icon, but it will cause tree shake error.
      // ```
      // This application cannot tree shake icons fonts. It has non-constant instances of IconData at the following locations:
      // Target release_macos_bundle_flutter_assets failed: Exception: Avoid non-constant invocations of IconData or try to build again with --no-tree-shake-icons.
      // ```
      //
      // trailingIcon: Icon(
      //     IconData(int.parse(ui.icon, radix: 16), fontFamily: 'MaterialIcons')),
      //
      // to-do: RustDesk translate or plugin translate ?
      child: Text(ui.text),
      ffi: ffi,
    );
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

  String? _getOption(OptionModel model, String key) {
    var v = model.value;
    if (v == null) {
      try {
        if (peerId.isEmpty) {
          v = bind.pluginGetSharedOption(id: pluginId, key: key);
        } else {
          v = bind.pluginGetSessionOption(id: pluginId, peer: peerId, key: key);
        }
      } catch (e) {
        debugPrint('Failed to get option "$key", $e');
        v = null;
      }
    }
    return v;
  }
}

void handleReloading(Map<String, dynamic> evt, String peer) {
  if (evt['id'] == null || evt['location'] == null) {
    return;
  }
  try {
    final ui = UiType.create(json.decode(evt['ui'] as String));
    if (ui != null) {
      addLocationUi(evt['location']!, evt['id']!, ui);
    }
  } catch (e) {
    debugPrint('Failed handleReloading, json decode of ui, $e ');
  }
}

void handleOption(Map<String, dynamic> evt, String peer) {
  updateOption(
      evt['location'], evt['id'], evt['peer'] ?? '', evt['key'], evt['value']);
}
