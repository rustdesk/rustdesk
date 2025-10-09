import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:toggle_switch/toggle_switch.dart';

class GestureIcons {
  static const String _family = 'gestureicons';

  GestureIcons._();

  static const IconData iconMouse = IconData(0xe65c, fontFamily: _family);
  static const IconData iconTabletTouch = IconData(0xe9ce, fontFamily: _family);
  static const IconData iconGestureFDrag =
      IconData(0xe686, fontFamily: _family);
  static const IconData iconMobileTouch = IconData(0xe9cd, fontFamily: _family);
  static const IconData iconGesturePress =
      IconData(0xe66c, fontFamily: _family);
  static const IconData iconGestureTap = IconData(0xe66f, fontFamily: _family);
  static const IconData iconGesturePinch =
      IconData(0xe66a, fontFamily: _family);
  static const IconData iconGesturePressHold =
      IconData(0xe66b, fontFamily: _family);
  static const IconData iconGestureFDragUpDown_ =
      IconData(0xe685, fontFamily: _family);
  static const IconData iconGestureFTap_ =
      IconData(0xe68e, fontFamily: _family);
  static const IconData iconGestureFSwipeRight =
      IconData(0xe68f, fontFamily: _family);
  static const IconData iconGestureFdoubleTap =
      IconData(0xe691, fontFamily: _family);
  static const IconData iconGestureFThreeFingers =
      IconData(0xe687, fontFamily: _family);
}

typedef OnTouchModeChange = void Function(bool);

class GestureHelp extends StatefulWidget {
  GestureHelp(
      {Key? key,
      required this.touchMode,
      required this.onTouchModeChange,
      required this.virtualMouseMode})
      : super(key: key);
  final bool touchMode;
  final OnTouchModeChange onTouchModeChange;
  final VirtualMouseMode virtualMouseMode;

  @override
  State<StatefulWidget> createState() =>
      _GestureHelpState(touchMode, virtualMouseMode);
}

class _GestureHelpState extends State<GestureHelp> {
  late int _selectedIndex;
  late bool _touchMode;
  final VirtualMouseMode _virtualMouseMode;

  _GestureHelpState(bool touchMode, VirtualMouseMode virtualMouseMode)
      : _virtualMouseMode = virtualMouseMode {
    _touchMode = touchMode;
    _selectedIndex = _touchMode ? 1 : 0;
  }

  @override
  Widget build(BuildContext context) {
    final size = MediaQuery.of(context).size;
    final space = 12.0;
    var width = size.width - 2 * space;
    final minWidth = 90;
    if (size.width > minWidth + 2 * space) {
      final n = (size.width / (minWidth + 2 * space)).floor();
      width = size.width / n - 2 * space;
    }
    return Center(
        child: Padding(
            padding: const EdgeInsets.symmetric(vertical: 12.0),
            child: Column(
              mainAxisAlignment: MainAxisAlignment.center,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Center(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      ToggleSwitch(
                        initialLabelIndex: _selectedIndex,
                        activeFgColor: Colors.white,
                        inactiveFgColor: Colors.white60,
                        activeBgColor: [MyTheme.accent],
                        inactiveBgColor: Theme.of(context).hintColor,
                        totalSwitches: 2,
                        minWidth: 150,
                        fontSize: 15,
                        iconSize: 18,
                        labels: [
                          translate("Mouse mode"),
                          translate("Touch mode")
                        ],
                        icons: [Icons.mouse, Icons.touch_app],
                        onToggle: (index) {
                          setState(() {
                            if (_selectedIndex != index) {
                              _selectedIndex = index ?? 0;
                              _touchMode = index == 0 ? false : true;
                              widget.onTouchModeChange(_touchMode);
                            }
                          });
                        },
                      ),
                      Transform.translate(
                        offset: const Offset(-10.0, 0.0),
                        child: Row(
                          mainAxisSize: MainAxisSize.min,
                          children: [
                            Checkbox(
                              value: _virtualMouseMode.showVirtualMouse,
                              onChanged: (value) async {
                                if (value == null) return;
                                await _virtualMouseMode.toggleVirtualMouse();
                                setState(() {});
                              },
                            ),
                            InkWell(
                              onTap: () async {
                                await _virtualMouseMode.toggleVirtualMouse();
                                setState(() {});
                              },
                              child: Text(translate('Show virtual mouse')),
                            ),
                          ],
                        ),
                      ),
                      if (_touchMode && _virtualMouseMode.showVirtualMouse)
                        Padding(
                          // Indent "Virtual mouse size"
                          padding: const EdgeInsets.only(left: 24.0),
                          child: SizedBox(
                            width: 260,
                            child: Column(
                              crossAxisAlignment: CrossAxisAlignment.start,
                              mainAxisSize: MainAxisSize.min,
                              children: [
                                Padding(
                                  padding: const EdgeInsets.only(
                                      top: 0.0, bottom: 0),
                                  child: Text(translate('Virtual mouse size')),
                                ),
                                Transform.translate(
                                  offset: Offset(-0.0, -6.0),
                                  child: Row(
                                    children: [
                                      Padding(
                                        padding:
                                            const EdgeInsets.only(left: 0.0),
                                        child: Text(translate('Small')),
                                      ),
                                      Expanded(
                                        child: Slider(
                                          value: _virtualMouseMode
                                              .virtualMouseScale,
                                          min: 0.8,
                                          max: 1.8,
                                          divisions: 10,
                                          onChanged: (value) {
                                            _virtualMouseMode
                                                .setVirtualMouseScale(value);
                                            setState(() {});
                                          },
                                        ),
                                      ),
                                      Padding(
                                        padding:
                                            const EdgeInsets.only(right: 16.0),
                                        child: Text(translate('Large')),
                                      ),
                                    ],
                                  ),
                                ),
                              ],
                            ),
                          ),
                        ),
                      if (!_touchMode && _virtualMouseMode.showVirtualMouse)
                        Transform.translate(
                          offset: const Offset(-10.0, -12.0),
                          child: Padding(
                              // Indent "Show virtual joystick"
                              padding: const EdgeInsets.only(left: 24.0),
                              child: Row(
                                mainAxisSize: MainAxisSize.min,
                                children: [
                                  Checkbox(
                                    value:
                                        _virtualMouseMode.showVirtualJoystick,
                                    onChanged: (value) async {
                                      if (value == null) return;
                                      await _virtualMouseMode
                                          .toggleVirtualJoystick();
                                      setState(() {});
                                    },
                                  ),
                                  InkWell(
                                    onTap: () async {
                                      await _virtualMouseMode
                                          .toggleVirtualJoystick();
                                      setState(() {});
                                    },
                                    child: Text(
                                        translate("Show virtual joystick")),
                                  ),
                                ],
                              )),
                        ),
                    ],
                  ),
                ),
                Container(
                    child: Wrap(
                  spacing: space,
                  runSpacing: 2 * space,
                  children: _touchMode
                      ? [
                          GestureInfo(
                              width,
                              GestureIcons.iconMobileTouch,
                              translate("One-Finger Tap"),
                              translate("Left Mouse")),
                          GestureInfo(
                              width,
                              GestureIcons.iconGesturePressHold,
                              translate("One-Long Tap"),
                              translate("Right Mouse")),
                          GestureInfo(
                              width,
                              GestureIcons.iconGestureFSwipeRight,
                              translate("One-Finger Move"),
                              translate("Mouse Drag")),
                          GestureInfo(
                              width,
                              GestureIcons.iconGestureFThreeFingers,
                              translate("Three-Finger vertically"),
                              translate("Mouse Wheel")),
                          GestureInfo(
                              width,
                              GestureIcons.iconGestureFDrag,
                              translate("Two-Finger Move"),
                              translate("Canvas Move")),
                          GestureInfo(
                              width,
                              GestureIcons.iconGesturePinch,
                              translate("Pinch to Zoom"),
                              translate("Canvas Zoom")),
                        ]
                      : [
                          GestureInfo(
                              width,
                              GestureIcons.iconMobileTouch,
                              translate("One-Finger Tap"),
                              translate("Left Mouse")),
                          GestureInfo(
                              width,
                              GestureIcons.iconGesturePressHold,
                              translate("One-Long Tap"),
                              translate("Right Mouse")),
                          GestureInfo(
                              width,
                              GestureIcons.iconGestureFSwipeRight,
                              translate("Double Tap & Move"),
                              translate("Mouse Drag")),
                          GestureInfo(
                              width,
                              GestureIcons.iconGestureFThreeFingers,
                              translate("Three-Finger vertically"),
                              translate("Mouse Wheel")),
                          GestureInfo(
                              width,
                              GestureIcons.iconGestureFDrag,
                              translate("Two-Finger Move"),
                              translate("Canvas Move")),
                          GestureInfo(
                              width,
                              GestureIcons.iconGesturePinch,
                              translate("Pinch to Zoom"),
                              translate("Canvas Zoom")),
                        ],
                )),
              ],
            )));
  }
}

class GestureInfo extends StatelessWidget {
  const GestureInfo(this.width, this.icon, this.fromText, this.toText,
      {Key? key})
      : super(key: key);

  final String fromText;
  final String toText;
  final IconData icon;
  final double width;

  final iconSize = 35.0;
  final iconColor = MyTheme.accent;

  @override
  Widget build(BuildContext context) {
    return Container(
        width: width,
        child: Column(
          children: [
            Icon(
              icon,
              size: iconSize,
              color: iconColor,
            ),
            SizedBox(height: 6),
            Text(fromText,
                textAlign: TextAlign.center,
                style:
                    TextStyle(fontSize: 9, color: Theme.of(context).hintColor)),
            SizedBox(height: 3),
            Text(toText,
                textAlign: TextAlign.center,
                style: TextStyle(
                    fontSize: 12,
                    color: Theme.of(context).textTheme.bodySmall?.color))
          ],
        ));
  }
}
