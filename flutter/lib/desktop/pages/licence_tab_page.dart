import 'package:flutter/material.dart';

class LicenceTabPage extends StatefulWidget {
  const LicenceTabPage({Key? key}) : super(key: key);

  @override
  State<LicenceTabPage> createState() => _LicenceTabPageState();
}

class _LicenceTabPageState extends State<LicenceTabPage>
    with WidgetsBindingObserver {
  @override
  void initState() {
    super.initState();
    // HardwareKeyboard.instance.addHandler(_handleKeyEvent);
    WidgetsBinding.instance.addObserver(this);
  }

  @override
  void dispose() {
    // HardwareKeyboard.instance.removeHandler(_handleKeyEvent);
    WidgetsBinding.instance.removeObserver(this);
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Container(
        child: Scaffold(
            backgroundColor: Theme.of(context).colorScheme.background,
            body: Center(child: CircularProgressIndicator())));
  }
}
