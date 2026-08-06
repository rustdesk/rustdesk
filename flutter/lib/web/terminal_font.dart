import 'dart:html' as html;
import 'dart:js' as js;
import 'dart:typed_data';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

bool _loadRequested = false;

/// When Google CDNs are unreachable, `index.html` sets
/// `window.rustdeskLocalFonts` and `GoogleFonts.robotoMono()` cannot download
/// the terminal font. Load the copy bundled with the web app instead,
/// registered under the family name google_fonts gives the terminal's
/// TextStyle ('RobotoMono_regular').
Future<void> loadLocalTerminalFontIfNeeded() async {
  if (_loadRequested || js.context['rustdeskLocalFonts'] != true) {
    return;
  }
  _loadRequested = true;
  try {
    final req = await html.HttpRequest.request(
      'fonts/RobotoMono-Regular.ttf',
      responseType: 'arraybuffer',
    );
    final data = ByteData.view(req.response as ByteBuffer);
    final loader = FontLoader('RobotoMono_regular')
      ..addFont(Future.value(data));
    await loader.load();
  } catch (e) {
    _loadRequested = false;
    debugPrint('Failed to load bundled Roboto Mono: $e');
  }
}
