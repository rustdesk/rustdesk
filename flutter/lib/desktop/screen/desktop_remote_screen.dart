import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/desktop/pages/connection_tab_page.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';
import 'package:provider/provider.dart';

/// multi-tab desktop remote screen
class DesktopRemoteScreen extends StatelessWidget {
  final Map<String, dynamic> params;

  const DesktopRemoteScreen({Key? key, required this.params}) : super(key: key);

  @override
  Widget build(BuildContext context) {
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
          title: 'RustDesk - Remote Desktop',
          theme: ThemeData(
            primarySwatch: Colors.blue,
            visualDensity: VisualDensity.adaptivePlatformDensity,
          ),
          home: ConnectionTabPage(
            params: params,
          ),
          navigatorObservers: [
            // FirebaseAnalyticsObserver(analytics: analytics),
            FlutterSmartDialog.observer
          ],
          builder: FlutterSmartDialog.init(
              builder: isAndroid
                  ? (_, child) => AccessibilityListener(
                        child: child,
                      )
                  : null)),
    );
  }
}
