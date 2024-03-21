import 'native/main.dart' if (dart.library.html) 'web/main.dart';

Future<void> main(List<String> args) async {
  await runMain(args);
}
