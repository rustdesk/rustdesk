import 'package:flutter_hbb/generated_bridge.dart';
import 'native_model.dart' if (dart.library.html) 'web_model.dart';

final platformFFI = PlatformFFI.instance;
final localeName = PlatformFFI.localeName;

RustdeskImpl get bind => platformFFI.ffiBind;
