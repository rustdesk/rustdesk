import 'package:flutter/material.dart';
import '../../common.dart' show MyTheme;

class IosKeyboardControls extends StatefulWidget {
  const IosKeyboardControls({super.key, required this.toolbarBuilder});

  final Widget Function(VoidCallback collapse) toolbarBuilder;

  @override
  State<IosKeyboardControls> createState() => _IosKeyboardControlsState();
}

class _IosKeyboardControlsState extends State<IosKeyboardControls> {
  bool _expanded = false;

  @override
  Widget build(BuildContext context) => MediaQuery.removePadding(
        context: context,
        removeBottom: true,
        child: SizedBox(
          height: kMinInteractiveDimension,
          child: _expanded
              ? widget.toolbarBuilder(() => setState(() => _expanded = false))
              : Align(
                  alignment: Alignment.centerRight,
                  child: Padding(
                    padding: const EdgeInsets.only(right: 16),
                    child: FloatingActionButton(
                      heroTag: null,
                      mini: true,
                      backgroundColor: MyTheme.accent,
                      onPressed: () => setState(() => _expanded = true),
                      child: const Icon(Icons.expand_less, color: Colors.white),
                    ),
                  ),
                ),
        ),
      );
}
