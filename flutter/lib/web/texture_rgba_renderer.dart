import 'dart:typed_data';

class TextureRgbaRenderer {
  Future<int> createTexture(int key) {
    throw UnimplementedError();
  }

  Future<bool> closeTexture(int key) {
    throw UnimplementedError();
  }

  Future<bool> onRgba(
      int key, Uint8List data, int height, int width, int strideAlign) {
    throw UnimplementedError();
  }

  Future<int> getTexturePtr(int key) {
    throw UnimplementedError();
  }
}
