import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'package:firebase_analytics/firebase_analytics.dart';
import 'package:firebase_analytics/observer.dart';
import 'package:firebase_core/firebase_core.dart';
import 'model.dart';
import 'home_page.dart';

Future<Null> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  await Firebase.initializeApp();
  runApp(App());
}

class App extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    final analytics = FirebaseAnalytics();
    return ChangeNotifierProvider.value(
        value: FFI.ffiModel,
        child: ChangeNotifierProvider.value(
            value: FFI.imageModel,
            child: ChangeNotifierProvider.value(
                value: FFI.cursorModel,
                child: ChangeNotifierProvider.value(
                    value: FFI.canvasModel,
                    child: MaterialApp(
                      title: 'RustDesk',
                      theme: ThemeData(
                        primarySwatch: Colors.blue,
                        visualDensity: VisualDensity.adaptivePlatformDensity,
                      ),
                      home: HomePage(title: 'RustDesk'),
                      navigatorObservers: [
                        FirebaseAnalyticsObserver(analytics: analytics),
                      ],
                    )))));
  }
}
