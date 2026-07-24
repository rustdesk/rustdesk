import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/mobile/widgets/dialog.dart' show showServerSettingsWithValue;
import 'package:flutter_hbb/models/platform_model.dart';

/// Check clipboard for valid server config on startup and show settings dialog if found.
///
/// This should be called from the home page's initState.
/// The [isMounted] callback is used to check if the widget is still mounted after async operations.
/// The [setState] callback is passed to the dialog for UI updates.
void checkAndShowClipboardServerConfig({
  required bool Function() isMounted,
  required void Function(VoidCallback) setState,
}) {
  final hideServer =
      bind.mainGetBuildinOption(key: kOptionHideServerSetting) == 'Y';
  if (hideServer) return;

  WidgetsBinding.instance.addPostFrameCallback((_) async {
    if (!isMounted()) return;

    try {
      final clipboardData = await Clipboard.getData(Clipboard.kTextPlain);
      if (!isMounted()) return;

      final config = tryDecodeClipboardServerConfig(clipboardData?.text);
      if (config != null) {
        if (bind.mainIsInstalled()) {
          final hasPermission = await callMainCheckSuperUserPermission();
          if (!hasPermission || !isMounted()) return;
        }
        // Clear clipboard after successful parsing
        await Clipboard.setData(const ClipboardData(text: ''));
        if (!isMounted()) return;
        showServerSettingsWithValue(config, gFFI.dialogManager, (fn) {
          if (isMounted()) setState(fn);
        });
      }
    } catch (e) {
      debugPrint('checkAndShowClipboardServerConfig error: $e');
    }
  });
}