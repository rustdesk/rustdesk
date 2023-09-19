import 'package:get/get.dart';
import 'package:texture_rgba_renderer/texture_rgba_renderer.dart';

import '../../common.dart';
import './platform_model.dart';

class RenderTexture {
  final RxInt textureId = RxInt(-1);
  int _textureKey = -1;
  SessionID? _sessionId;
  static final useTextureRender = bind.mainUseTextureRender();

  final textureRenderer = TextureRgbaRenderer();

  RenderTexture();

  create(SessionID sessionId) {
    if (useTextureRender) {
      _textureKey = bind.getNextTextureKey();
      _sessionId = sessionId;

      textureRenderer.createTexture(_textureKey).then((id) async {
        if (id != -1) {
          final ptr = await textureRenderer.getTexturePtr(_textureKey);
          platformFFI.registerTexture(sessionId, ptr);
          textureId.value = id;
        }
      });
    }
  }

  destroy(bool unregisterTexture) async {
    if (useTextureRender && _textureKey != -1 && _sessionId != null) {
      if (unregisterTexture) {
        platformFFI.registerTexture(_sessionId!, 0);
      }
      await textureRenderer.closeTexture(_textureKey);
      _textureKey = -1;
    }
  }
}
