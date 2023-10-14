import 'package:get/get.dart';
import 'package:texture_rgba_renderer/texture_rgba_renderer.dart';

import '../../common.dart';
import './platform_model.dart';

final useTextureRender = bind.mainUseTextureRender();

class RenderTexture {
  final RxInt textureId = RxInt(-1);
  int _textureKey = -1;
  int _display = 0;
  SessionID? _sessionId;

  final textureRenderer = TextureRgbaRenderer();

  RenderTexture();

  int get display => _display;

  create(int d, SessionID sessionId) {
    if (useTextureRender) {
      _display = d;
      _textureKey = bind.getNextTextureKey();
      _sessionId = sessionId;

      textureRenderer.createTexture(_textureKey).then((id) async {
        if (id != -1) {
          final ptr = await textureRenderer.getTexturePtr(_textureKey);
          platformFFI.registerTexture(sessionId, display, ptr);
          textureId.value = id;
        }
      });
    }
  }

  destroy(bool unregisterTexture) async {
    if (useTextureRender && _textureKey != -1 && _sessionId != null) {
      if (unregisterTexture) {
        platformFFI.registerTexture(_sessionId!, display, 0);
      }
      await textureRenderer.closeTexture(_textureKey);
      _textureKey = -1;
    }
  }
}
