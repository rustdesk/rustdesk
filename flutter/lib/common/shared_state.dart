import 'package:get/get.dart';

import '../consts.dart';

// TODO: A lot of dup code.

class PrivacyModeState {
  static String tag(String id) => 'privacy_mode_$id';

  static void init(String id) {
    final key = tag(id);
    if (!Get.isRegistered(tag: key)) {
      final RxBool state = false.obs;
      Get.put(state, tag: key);
    }
  }

  static void delete(String id) {
    final key = tag(id);
    if (Get.isRegistered(tag: key)) {
      Get.delete(tag: key);
    }
  }

  static RxBool find(String id) => Get.find<RxBool>(tag: tag(id));
}

class BlockInputState {
  static String tag(String id) => 'block_input_$id';

  static void init(String id) {
    final key = tag(id);
    if (!Get.isRegistered(tag: key)) {
      final RxBool state = false.obs;
      Get.put(state, tag: key);
    }
  }

  static void delete(String id) {
    final key = tag(id);
    if (Get.isRegistered(tag: key)) {
      Get.delete(tag: key);
    }
  }

  static RxBool find(String id) => Get.find<RxBool>(tag: tag(id));
}

class CurrentDisplayState {
  static String tag(String id) => 'current_display_$id';

  static void init(String id) {
    final key = tag(id);
    if (!Get.isRegistered(tag: key)) {
      final RxInt state = RxInt(0);
      Get.put(state, tag: key);
    }
  }

  static void delete(String id) {
    final key = tag(id);
    if (Get.isRegistered(tag: key)) {
      Get.delete(tag: key);
    }
  }

  static RxInt find(String id) => Get.find<RxInt>(tag: tag(id));
}

class ConnectionType {
  final Rx<String> _secure = kInvalidValueStr.obs;
  final Rx<String> _direct = kInvalidValueStr.obs;

  Rx<String> get secure => _secure;
  Rx<String> get direct => _direct;

  static String get strSecure => 'secure';
  static String get strInsecure => 'insecure';
  static String get strDirect => '';
  static String get strIndirect => '_relay';

  void setSecure(bool v) {
    _secure.value = v ? strSecure : strInsecure;
  }

  void setDirect(bool v) {
    _direct.value = v ? strDirect : strIndirect;
  }

  bool isValid() {
    return _secure.value != kInvalidValueStr &&
        _direct.value != kInvalidValueStr;
  }
}

class ConnectionTypeState {
  static String tag(String id) => 'connection_type_$id';

  static void init(String id) {
    final key = tag(id);
    if (!Get.isRegistered(tag: key)) {
      final ConnectionType collectionType = ConnectionType();
      Get.put(collectionType, tag: key);
    }
  }

  static void delete(String id) {
    final key = tag(id);
    if (Get.isRegistered(tag: key)) {
      Get.delete(tag: key);
    }
  }

  static ConnectionType find(String id) =>
      Get.find<ConnectionType>(tag: tag(id));
}

class ShowRemoteCursorState {
  static String tag(String id) => 'show_remote_cursor_$id';

  static void init(String id) {
    final key = tag(id);
    if (!Get.isRegistered(tag: key)) {
      final RxBool state = false.obs;
      Get.put(state, tag: key);
    }
  }

  static void delete(String id) {
    final key = tag(id);
    if (Get.isRegistered(tag: key)) {
      Get.delete(tag: key);
    }
  }

  static RxBool find(String id) => Get.find<RxBool>(tag: tag(id));
}

class KeyboardEnabledState {
  static String tag(String id) => 'keyboard_enabled_$id';

  static void init(String id) {
    final key = tag(id);
    if (!Get.isRegistered(tag: key)) {
      // Server side, default true
      final RxBool state = true.obs;
      Get.put(state, tag: key);
    }
  }

  static void delete(String id) {
    final key = tag(id);
    if (Get.isRegistered(tag: key)) {
      Get.delete(tag: key);
    }
  }

  static RxBool find(String id) => Get.find<RxBool>(tag: tag(id));
}

class RemoteCursorMovedState {
  static String tag(String id) => 'remote_cursor_moved_$id';

  static void init(String id) {
    final key = tag(id);
    if (!Get.isRegistered(tag: key)) {
      // Server side, default true
      final RxBool state = false.obs;
      Get.put(state, tag: key);
    }
  }

  static void delete(String id) {
    final key = tag(id);
    if (Get.isRegistered(tag: key)) {
      Get.delete(tag: key);
    }
  }

  static RxBool find(String id) => Get.find<RxBool>(tag: tag(id));
}
