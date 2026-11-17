import 'package:flutter_hbb/common.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('request-approval links preserve their connection type', () {
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
            Uri.parse('rustdesk://$type/123456789?request-approval=true')),
        ['--$type', '123456789', '--request-approval'],
      );
    }
    expect(
      urlLinkToCmdArgs(Uri.parse('rustdesk://123456789?request-approval=true')),
      ['--connect', '123456789', '--request-approval'],
    );
  });

  test('omitted and empty passwords retain automatic login', () {
    expect(urlLinkToCmdArgs(Uri.parse('rustdesk://123456789')),
        ['--connect', '123456789']);
    for (final query in ['password=', 'password=&request-approval=false']) {
      expect(urlLinkToCmdArgs(Uri.parse('rustdesk://123456789?$query')),
          ['--connect', '123456789', '--password', '']);
    }
  });

  test('request-approval ignores an explicit password in either order', () {
    for (final query in [
      'password=provided&request-approval=true',
      'request-approval=true&password=provided',
    ]) {
      expect(urlLinkToCmdArgs(Uri.parse('rustdesk://123456789?$query')),
          ['--connect', '123456789', '--request-approval']);
    }
  });
}
