import 'native_model.dart' if (dart.library.html) 'web_model.dart';
import 'package:flutter_hbb/generated_bridge.dart'
    if (dart.library.html) 'package:flutter_hbb/web/bridge.dart';

final platformFFI = PlatformFFI.instance;
final localeName = PlatformFFI.localeName;

RustdeskImpl get bind => platformFFI.ffiBind;
