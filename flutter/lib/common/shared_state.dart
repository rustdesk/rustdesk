import 'package:get/get.dart';

import '../consts.dart';

class PrivacyModeState {
  static String tag(String id) => 'privacy_mode_$id';

  static void init(String id) {
    final RxBool state = false.obs;
    Get.put(state, tag: tag(id));
  }

  static void delete(String id) => Get.delete(tag: tag(id));
  static RxBool find(String id) => Get.find<RxBool>(tag: tag(id));
}

class BlockInputState {
  static String tag(String id) => 'block_input_$id';

  static void init(String id) {
    final RxBool state = false.obs;
    Get.put(state, tag: tag(id));
  }

  static void delete(String id) => Get.delete(tag: tag(id));
  static RxBool find(String id) => Get.find<RxBool>(tag: tag(id));
}

class CurrentDisplayState {
  static String tag(String id) => 'current_display_$id';

  static void init(String id) {
    final RxInt state = RxInt(0);
    Get.put(state, tag: tag(id));
  }

  static void delete(String id) => Get.delete(tag: tag(id));
  static RxInt find(String id) => Get.find<RxInt>(tag: tag(id));
}

class ConnectionType {
  final Rx<String> _secure = kInvalidValueStr.obs;
  final Rx<String> _direct = kInvalidValueStr.obs;

  Rx<String> get secure => _secure;
  Rx<String> get direct => _direct;

  void setSecure(bool v) {
    _secure.value = v ? 'secure' : 'insecure';
  }

  void setDirect(bool v) {
    _direct.value = v ? '' : '_relay';
  }

  bool isValid() {
    return _secure.value != kInvalidValueStr &&
        _direct.value != kInvalidValueStr;
  }
}

class ConnectionTypeState {
  static String tag(String id) => 'connection_type_$id';

  static void init(String id) {
    final ConnectionType collectionType = ConnectionType();
    Get.put(collectionType, tag: tag(id));
  }

  static void delete(String id) => Get.delete(tag: tag(id));
  static ConnectionType find(String id) =>
      Get.find<ConnectionType>(tag: tag(id));
}
