import 'package:flutter/foundation.dart';
import 'package:flutter_hbb/models/platform_model.dart';

class SettingsStore extends ChangeNotifier {
  static const _ns = 'tabby:';

  String? get idServer => _read('id_server');
  String? get relayServer => _read('relay_server');
  String? get publicKey => _read('public_key');

  bool get leftHanded => _read('left_handed') == '1';
  double get scrollSensitivity =>
      double.tryParse(_read('scroll_sensitivity') ?? '') ?? 1.0;
  bool get scrollInverted => _read('scroll_inverted') == '1';
  double get macroBarTopOffset =>
      double.tryParse(_read('macro_bar_top') ?? '') ?? 0.0;
  bool get macroBarCollapsed => _read('macro_bar_collapsed') == '1';
  // Default true: preserves shipped behavior; users can opt out from Settings.
  bool get rememberLastDisplay => _read('remember_last_display') != '0';
  bool get rememberLastZoom => _read('remember_last_zoom') != '0';

  Future<void> setServer({
    required String idServer,
    required String relayServer,
    required String publicKey,
  }) async {
    await bind.mainSetLocalOption(
        key: '${_ns}id_server', value: idServer);
    await bind.mainSetLocalOption(
        key: '${_ns}relay_server', value: relayServer);
    await bind.mainSetLocalOption(
        key: '${_ns}public_key', value: publicKey);
    notifyListeners();
  }

  Future<void> setLeftHanded(bool value) =>
      _write('left_handed', value ? '1' : '0');

  Future<void> setScrollSensitivity(double value) =>
      _write('scroll_sensitivity', value.toStringAsFixed(2));

  Future<void> setScrollInverted(bool value) =>
      _write('scroll_inverted', value ? '1' : '0');

  Future<void> setMacroBarTopOffset(double value) =>
      _write('macro_bar_top', value.toStringAsFixed(1));

  Future<void> setMacroBarCollapsed(bool value) =>
      _write('macro_bar_collapsed', value ? '1' : '0');

  Future<void> setRememberLastDisplay(bool value) =>
      _write('remember_last_display', value ? '1' : '0');

  Future<void> setRememberLastZoom(bool value) =>
      _write('remember_last_zoom', value ? '1' : '0');

  String? _read(String key) {
    final v = bind.mainGetLocalOption(key: '$_ns$key');
    return v.isEmpty ? null : v;
  }

  Future<void> _write(String key, String value) async {
    await bind.mainSetLocalOption(key: '$_ns$key', value: value);
    notifyListeners();
  }
}

final settingsStore = SettingsStore();
