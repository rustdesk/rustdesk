import 'package:flutter/material.dart';
import '../../common.dart' show MyTheme;

class IosKeyboardControls extends StatefulWidget {
  const IosKeyboardControls({
    super.key,
    required this.toolbarBuilder,
    required this.auxiliaryBuilder,
  });

  final Widget Function(VoidCallback collapse, VoidCallback toggleAuxiliary)
  toolbarBuilder;
  final Widget Function(bool requested) auxiliaryBuilder;

  @override
  State<IosKeyboardControls> createState() => _IosKeyboardControlsState();
}

class _IosKeyboardControlsState extends State<IosKeyboardControls> {
  bool _expanded = false;
  bool _showAuxiliary = false;

  @override
  Widget build(BuildContext context) => MediaQuery.removePadding(
    context: context,
    removeTop: true,
    removeBottom: true,
    child: Align(
      alignment: Alignment.bottomCenter,
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Flexible(
            child: SingleChildScrollView(
              child: widget.auxiliaryBuilder(_expanded && _showAuxiliary),
            ),
          ),
          SizedBox(
            height: kMinInteractiveDimension,
            child: _expanded
                ? widget.toolbarBuilder(
                    () => setState(() => _expanded = false),
                    () => setState(() => _showAuxiliary = !_showAuxiliary),
                  )
                : Align(
                    alignment: Alignment.centerRight,
                    child: Padding(
                      padding: const EdgeInsets.only(right: 16),
                      child: FloatingActionButton(
                        heroTag: null,
                        mini: true,
                        backgroundColor: MyTheme.accent,
                        onPressed: () => setState(() => _expanded = true),
                        child: const Icon(
                          Icons.expand_less,
                          color: Colors.white,
                        ),
                      ),
                    ),
                  ),
          ),
        ],
      ),
    ),
  );
}
