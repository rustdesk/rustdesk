import 'package:flutter_hbb/utils/app_dir.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  group('appDirAfterLookupFailure', () {
    // Without xdg-user-dir the documents lookup throws on Linux, and an empty
    // app dir keeps recent and favorite peers from loading (#16622).
    test('uses HOME on Linux', () {
      expect(
          appDirAfterLookupFailure(
              isLinux: true, environment: {'HOME': '/home/ada'}),
          '/home/ada');
    });

    test('stays empty on Linux without HOME', () {
      expect(appDirAfterLookupFailure(isLinux: true, environment: {}), '');
    });

    test('stays empty on other platforms', () {
      expect(
          appDirAfterLookupFailure(
              isLinux: false, environment: {'HOME': '/Users/ada'}),
          '');
    });
  });
}
