import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_hbb/consts.dart';

/// Aplica SOLO id-server y key al inicio (no toca relay).
/// Llama a esta función ANTES de runApp().
Future<void> configureCustomServer() async {
  // Globales
  await bind.mainSetOption(key: 'id-server', value: kCustomIdServer);
  await bind.mainSetOption(key: 'key',       value: kCustomPubKey);

  // Locales (refuerzo para que persista en el perfil del usuario)
  await bind.mainSetLocalOption(key: 'id-server', value: kCustomIdServer);
  await bind.mainSetLocalOption(key: 'key',       value: kCustomPubKey);

  // ⚠️ Deliberadamente NO tocamos 'relay-server'
}
