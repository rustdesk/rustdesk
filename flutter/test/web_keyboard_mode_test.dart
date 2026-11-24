@TestOn('browser')
library;

import 'dart:convert';
import 'dart:js' as js;

import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:uuid/uuid.dart';

class _Session extends Fake implements FFI {
  @override
  final sessionId = const Uuid().v4obj();
}

void main() {
  for (final (preferred, wayland, expected) in [
    ('legacy', false, 'legacy'),
    ('translate', false, 'map'),
    ('legacy', true, 'map'),
  ]) {
    test(
        'Input source 2 with $preferred and Wayland=$wayland selects $expected',
        () async {
      var mode = preferred;
      js.context['isMobile'] = js.allowInterop(() => false);
      js.context['getByName'] = js.allowInterop((String name, String key) {
        if (key == 'input-source') return 'Input source 2';
        if (key == 'keyboard_mode') return mode;
        return '';
      });
      js.context['setByName'] = js.allowInterop((String name, String value) {
        if (name == 'option:session') mode = jsonDecode(value)['value'];
      });
      addTearDown(() {
        for (final name in ['isMobile', 'getByName', 'setByName']) {
          js.context.deleteProperty(name);
        }
      });

      final session = _Session();
      final model = FfiModel(WeakReference(session));
      model.pi.platformAdditions['is_wayland'] = wayland;
      await model.checkDesktopKeyboardMode();
      expect(mode, expected);
    });
  }
}
