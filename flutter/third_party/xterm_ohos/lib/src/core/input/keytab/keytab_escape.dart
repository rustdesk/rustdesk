final _esc = String.fromCharCode(0x1b);

String keytabUnescape(String str) {
  str = str
      .replaceAll(r'\E', _esc)
      .replaceAll(r'\\', '\\')
      .replaceAll(r'\"', '"')
      .replaceAll(r'\t', '\t')
      .replaceAll(r'\r', '\r')
      .replaceAll(r'\n', '\n')
      .replaceAll(r'\b', '\b');

  final hexPattern = RegExp(r'\\x([0-9a-fA-F][0-9a-fA-F])');
  str = str.replaceAllMapped(hexPattern, (match) {
    final hexString = match.group(1)!;
    final hexValue = int.parse(hexString, radix: 16);
    return String.fromCharCode(hexValue);
  });

  return str;
}
