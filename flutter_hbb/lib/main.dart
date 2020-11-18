import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'common.dart';
import 'home_page.dart';

void main() {
  runApp(App());
}

class App extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return ChangeNotifierProvider.value(
        value: FFI.ffiModel,
        child: ChangeNotifierProvider.value(
            value: FFI.imageModel,
            child: ChangeNotifierProvider.value(
                value: FFI.cursorModel,
                child: MaterialApp(
                  title: 'RustDesk',
                  theme: ThemeData(
                    primarySwatch: Colors.blue,
                    visualDensity: VisualDensity.adaptivePlatformDensity,
                  ),
                  home: HomePage(title: 'RustDesk'),
                ))));
  }
}
