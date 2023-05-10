import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_hbb/plugin/model.dart';
import 'package:flutter_hbb/plugin/common.dart';
import 'package:get/get.dart';

import '../manager.dart';
import './desc_ui.dart';

// to-do: use settings from desktop_setting_page.dart
const double _kCardFixedWidth = 540;
const double _kCardLeftMargin = 15;
const double _kContentHMargin = 15;
const double _kTitleFontSize = 20;
const double _kVersionFontSize = 12;

class DesktopSettingsCard extends StatefulWidget {
  final PluginInfo plugin;
  DesktopSettingsCard({
    Key? key,
    required this.plugin,
  }) : super(key: key);

  @override
  State<DesktopSettingsCard> createState() => _DesktopSettingsCardState();
}

class _DesktopSettingsCardState extends State<DesktopSettingsCard> {
  PluginInfo get plugin => widget.plugin;
  bool get installed => plugin.installedVersion.isNotEmpty;

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        Flexible(
          child: SizedBox(
            width: _kCardFixedWidth,
            child: Card(
              child: Column(
                children: [
                  header(),
                  body(),
                ],
              ).marginOnly(bottom: 10),
            ).marginOnly(left: _kCardLeftMargin, top: 15),
          ),
        ),
      ],
    );
  }

  Widget header() {
    return Row(
      children: [
        headerNameVersion(),
        headerInstallEnable(),
      ],
    ).marginOnly(
      left: _kContentHMargin,
      top: 10,
      bottom: 10,
      right: _kContentHMargin,
    );
  }

  Widget headerNameVersion() {
    return Expanded(
      child: Row(
        children: [
          Text(
            translate(widget.plugin.meta.name),
            textAlign: TextAlign.start,
            style: const TextStyle(
              fontSize: _kTitleFontSize,
            ),
          ),
          SizedBox(
            width: 5,
          ),
          Text(
            plugin.meta.version,
            textAlign: TextAlign.start,
            style: const TextStyle(
              fontSize: _kVersionFontSize,
            ),
          )
        ],
      ),
    );
  }

  Widget headerButton(String label, VoidCallback onPressed) {
    return Container(
      child: ElevatedButton(
        onPressed: onPressed,
        child: Text(label),
      ),
    );
  }

  Widget headerInstallEnable() {
    final installButton = headerButton(installed ? 'uninstall' : 'install', () {
      bind.pluginInstall(
        id: plugin.meta.id,
        b: !installed,
      );
    });

    if (installed) {
      final needUpdate =
          plugin.installedVersion.compareTo(plugin.meta.version) < 0;
      final updateButton = needUpdate
          ? headerButton('update', () {
              bind.pluginInstall(
                id: plugin.meta.id,
                b: !installed,
              );
            })
          : Container();

      final isEnabled = bind.pluginIsEnabled(id: plugin.meta.id);
      final enableButton = !installed
          ? Container()
          : headerButton(isEnabled ? 'disable' : 'enable', () {
              if (isEnabled) {
                clearPlugin(plugin.meta.id);
              }
              bind.pluginEnable(id: plugin.meta.id, v: !isEnabled);
              setState(() {});
            });
      return Row(
        children: [
          updateButton,
          SizedBox(
            width: 10,
          ),
          installButton,
          SizedBox(
            width: 10,
          ),
          enableButton,
        ],
      );
    } else {
      return installButton;
    }
  }

  Widget body() {
    return Column(children: [
      author(),
      description(),
      more(),
    ]).marginOnly(
      left: _kCardLeftMargin,
      top: 4,
      right: _kContentHMargin,
    );
  }

  Widget author() {
    return Align(
      alignment: Alignment.centerLeft,
      child: Text(plugin.meta.author),
    );
  }

  Widget description() {
    return Align(
      alignment: Alignment.centerLeft,
      child: Text(plugin.meta.description),
    );
  }

  Widget more() {
    if (!installed) {
      return Container();
    }

    final List<Widget> children = [];
    final model = getPluginModel(kLocationHostMainPlugin, plugin.meta.id);
    if (model != null) {
      children.add(PluginItem(
        pluginId: plugin.meta.id,
        peerId: '',
        location: kLocationHostMainPlugin,
        pluginModel: model,
        isMenu: false,
      ));
    }
    return ExpansionTile(
      title: Text('Options'),
      controlAffinity: ListTileControlAffinity.leading,
      children: children,
    );
  }
}
