import 'package:flutter/foundation.dart';

/// True while the Android activity is showing the outgoing remote session in
/// picture-in-picture. Updated from [androidChannelInit]. Desktop, iOS and web
/// leave it false.
final ValueNotifier<bool> androidInPip = ValueNotifier<bool>(false);
