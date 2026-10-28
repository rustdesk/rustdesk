import 'package:flutter_hbb/common.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('without-password links preserve their connection type', () {
    for (final type in [
      'connect',
      'play',
      'file-transfer',
      'view-camera',
      'port-forward',
      'rdp',
      'terminal',
      'terminal-admin',
    ]) {
      expect(
        urlLinkToCmdArgs(
            Uri.parse('rustdesk://$type/123456789?without-password=true')),
        ['--$type', '123456789', '--without-password'],
      );
    }
    expect(
      urlLinkToCmdArgs(Uri.parse('rustdesk://123456789?without-password=true')),
      ['--connect', '123456789', '--without-password'],
    );
  });

  test('omitted and empty passwords retain automatic login', () {
    expect(urlLinkToCmdArgs(Uri.parse('rustdesk://123456789')),
        ['--connect', '123456789']);
    for (final query in ['password=', 'password=&without-password=false']) {
      expect(urlLinkToCmdArgs(Uri.parse('rustdesk://123456789?$query')),
          ['--connect', '123456789', '--password', '']);
    }
  });

  test('without-password ignores an explicit password in either order', () {
    for (final query in [
      'password=provided&without-password=true',
      'without-password=true&password=provided',
    ]) {
      expect(urlLinkToCmdArgs(Uri.parse('rustdesk://123456789?$query')),
          ['--connect', '123456789', '--without-password']);
    }
  });
}
