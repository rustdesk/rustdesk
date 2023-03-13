import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:toggle_switch/toggle_switch.dart';

class GestureIcons {
  static const String _family = 'gestureicons';

  GestureIcons._();

  static const IconData icon_mouse = IconData(0xe65c, fontFamily: _family);
  static const IconData icon_Tablet_Touch =
      IconData(0xe9ce, fontFamily: _family);
  static const IconData icon_gesture_f_drag =
      IconData(0xe686, fontFamily: _family);
  static const IconData icon_Mobile_Touch =
      IconData(0xe9cd, fontFamily: _family);
  static const IconData icon_gesture_press =
      IconData(0xe66c, fontFamily: _family);
  static const IconData icon_gesture_tap =
      IconData(0xe66f, fontFamily: _family);
  static const IconData icon_gesture_pinch =
      IconData(0xe66a, fontFamily: _family);
  static const IconData icon_gesture_press_hold =
      IconData(0xe66b, fontFamily: _family);
  static const IconData icon_gesture_f_drag_up_down_ =
      IconData(0xe685, fontFamily: _family);
  static const IconData icon_gesture_f_tap_ =
      IconData(0xe68e, fontFamily: _family);
  static const IconData icon_gesture_f_swipe_right =
      IconData(0xe68f, fontFamily: _family);
  static const IconData icon_gesture_f_double_tap =
      IconData(0xe691, fontFamily: _family);
  static const IconData icon_gesture_f_three_fingers =
      IconData(0xe687, fontFamily: _family);
}

typedef OnTouchModeChange = void Function(bool);

class GestureHelp extends StatefulWidget {
  GestureHelp(
      {Key? key, required this.touchMode, required this.onTouchModeChange})
      : super(key: key);
  final bool touchMode;
  final OnTouchModeChange onTouchModeChange;

  @override
  State<StatefulWidget> createState() => _GestureHelpState();
}

class _GestureHelpState extends State<GestureHelp> {
  var _selectedIndex;
  var _touchMode;

  @override
  void initState() {
    _touchMode = widget.touchMode;
    _selectedIndex = _touchMode ? 1 : 0;
    super.initState();
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
              crossAxisAlignment: CrossAxisAlignment.center,
              children: <Widget>[
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
                  labels: [translate("Mouse mode"), translate("Touch mode")],
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
                const SizedBox(height: 30),
                Container(
                    child: Wrap(
                  spacing: space,
                  runSpacing: 2 * space,
                  children: _touchMode
                      ? [
                          GestureInfo(
                              width,
                              GestureIcons.icon_Mobile_Touch,
                              translate("One-Finger Tap"),
                              translate("Left Mouse")),
                          GestureInfo(
                              width,
                              GestureIcons.icon_gesture_press_hold,
                              translate("One-Long Tap"),
                              translate("Right Mouse")),
                          GestureInfo(
                              width,
                              GestureIcons.icon_gesture_f_swipe_right,
                              translate("One-Finger Move"),
                              translate("Mouse Drag")),
                          GestureInfo(
                              width,
                              GestureIcons.icon_gesture_f_three_fingers,
                              translate("Three-Finger vertically"),
                              translate("Mouse Wheel")),
                          GestureInfo(
                              width,
                              GestureIcons.icon_gesture_f_drag,
                              translate("Two-Finger Move"),
                              translate("Canvas Move")),
                          GestureInfo(
                              width,
                              GestureIcons.icon_gesture_pinch,
                              translate("Pinch to Zoom"),
                              translate("Canvas Zoom")),
                        ]
                      : [
                          GestureInfo(
                              width,
                              GestureIcons.icon_Mobile_Touch,
                              translate("One-Finger Tap"),
                              translate("Left Mouse")),
                          GestureInfo(
                              width,
                              GestureIcons.icon_gesture_press_hold,
                              translate("One-Long Tap"),
                              translate("Right Mouse")),
                          GestureInfo(
                              width,
                              GestureIcons.icon_gesture_f_swipe_right,
                              translate("Double Tap & Move"),
                              translate("Mouse Drag")),
                          GestureInfo(
                              width,
                              GestureIcons.icon_gesture_f_three_fingers,
                              translate("Three-Finger vertically"),
                              translate("Mouse Wheel")),
                          GestureInfo(
                              width,
                              GestureIcons.icon_gesture_f_drag,
                              translate("Two-Finger Move"),
                              translate("Canvas Move")),
                          GestureInfo(
                              width,
                              GestureIcons.icon_gesture_pinch,
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
