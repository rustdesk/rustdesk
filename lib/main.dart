import 'package:flutter/material.dart';
import 'package:flutter_easyloading/flutter_easyloading.dart';
import 'package:provider/provider.dart';
import 'package:firebase_analytics/firebase_analytics.dart';
import 'package:firebase_core/firebase_core.dart';
import 'common.dart';
import 'models/model.dart';
import 'pages/home_page.dart';
import 'pages/server_page.dart';
import 'pages/settings_page.dart';

Future<Null> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  var a = FFI.ffiModel.init();
  var b = Firebase.initializeApp();
  await a;
  await b;
  refreshCurrentUser();
  EasyLoading.instance.loadingStyle = EasyLoadingStyle.light;
  toAndroidChannelInit();
  runApp(App());
}

class App extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    final analytics = FirebaseAnalytics.instance;
    return MultiProvider(
      providers: [
        ChangeNotifierProvider.value(value: FFI.ffiModel),
        ChangeNotifierProvider.value(value: FFI.imageModel),
        ChangeNotifierProvider.value(value: FFI.cursorModel),
        ChangeNotifierProvider.value(value: FFI.canvasModel),
      ],
      child: MaterialApp(
        navigatorKey: globalKey,
        debugShowCheckedModeBanner: false,
        title: 'RustDesk',
        theme: ThemeData(
          primarySwatch: Colors.blue,
          visualDensity: VisualDensity.adaptivePlatformDensity,
        ),
        home: !isAndroid ? WebHomePage() : HomePage(),
        navigatorObservers: [
          FirebaseAnalyticsObserver(analytics: analytics),
        ],
        builder: isAndroid
            ? (_, child) {
                return AccessibilityListener(
                  child: FlutterEasyLoading(child: child),
                );
              }
            : EasyLoading.init(),
      ),
    );
  }
}
