/// The app dir to use when `getApplicationDocumentsDirectory()` fails.
///
/// On Linux that path comes from `xdg-user-dir`, which minimal installs lack.
/// It prints $HOME when no Documents dir is set, so use that: recent and
/// favorite peers don't load while the app dir is empty.
String appDirAfterLookupFailure({
  required bool isLinux,
  required Map<String, String> environment,
}) =>
    isLinux ? environment['HOME'] ?? '' : '';
