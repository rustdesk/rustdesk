import 'package:flutter/material.dart';
import '../../common.dart' show MyTheme;

class IosKeyboardControls extends StatefulWidget {
  const IosKeyboardControls({
    super.key,
    required this.toolbarBuilder,
    required this.auxiliaryBuilder,
    required this.keyboardVisible,
    this.child,
  });

  final Widget Function(VoidCallback collapse, VoidCallback toggleAuxiliary)
      toolbarBuilder;
  final Widget Function(bool requested) auxiliaryBuilder;
  final bool keyboardVisible;
  final Widget? child;

  @override
  State<IosKeyboardControls> createState() => _IosKeyboardControlsState();
}

class _IosKeyboardControlsState extends State<IosKeyboardControls> {
  late bool _expanded = !widget.keyboardVisible;
  bool _showAuxiliary = false;

  @override
  void didUpdateWidget(IosKeyboardControls oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.keyboardVisible != widget.keyboardVisible) {
      _expanded = !widget.keyboardVisible;
    }
  }

  @override
  Widget build(BuildContext context) => MediaQuery.removePadding(
        context: context,
        removeBottom: widget.keyboardVisible,
        child: Material(
          type: MaterialType.transparency,
          child: Column(
            children: [
              Expanded(
                child: Stack(
                  fit: StackFit.expand,
                  children: [
                    if (widget.child != null) widget.child!,
                    Align(
                      alignment: Alignment.topCenter,
                      child: SafeArea(
                        bottom: false,
                        child: SingleChildScrollView(
                          child: widget
                              .auxiliaryBuilder(_expanded && _showAuxiliary),
                        ),
                      ),
                    ),
                  ],
                ),
              ),
              SafeArea(
                top: false,
                child: SizedBox(
                  height: kMinInteractiveDimension,
                  child: _expanded
                      ? Scaffold(
                          backgroundColor: Colors.transparent,
                          resizeToAvoidBottomInset: false,
                          body: widget.toolbarBuilder(
                            () => setState(() => _expanded = false),
                            () => setState(
                                () => _showAuxiliary = !_showAuxiliary),
                          ),
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
                              child: const Icon(Icons.expand_less,
                                  color: Colors.white),
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
