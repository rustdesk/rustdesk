import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:toggle_switch/toggle_switch.dart';

import '../models/model.dart';

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
    return Center(
        child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 10),
            child: Column(
              mainAxisAlignment: MainAxisAlignment.center,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                ToggleSwitch(
                  initialLabelIndex: _selectedIndex,
                  inactiveBgColor: MyTheme.darkGray,
                  totalSwitches: 2,
                  minWidth: 150,
                  fontSize: 15,
                  iconSize: 18,
                  labels: [translate("TouchPad mode"), translate("Touch mode")],
                  icons: [Icons.mouse, Icons.touch_app],
                  onToggle: (index) {
                    debugPrint(index.toString());
                    setState(() {
                      if (_selectedIndex != index) {
                        _selectedIndex = index ?? 0;
                        _touchMode = index == 0 ? false : true;
                        widget.onTouchModeChange(_touchMode);
                      }
                    });
                  },
                ),
                const SizedBox(height: 15),
                Container(
                    child: Column(
                  mainAxisAlignment: MainAxisAlignment.start,
                  crossAxisAlignment: CrossAxisAlignment.center,
                  children: _touchMode
                      ? [
                          GestureInfo(
                              GestureIcons.icon_Mobile_Touch,
                              translate("One-Finger Tap"),
                              translate("Left Mouse")),
                          GestureInfo(
                              GestureIcons.icon_gesture_press_hold,
                              translate("One-Long Tap"),
                              translate("Right Mouse")),
                          GestureInfo(
                              GestureIcons.icon_gesture_f_swipe_right,
                              translate("One-Finger Move"),
                              translate("Mouse Drag")),
                          GestureInfo(
                              GestureIcons.icon_gesture_f_drag_up_down_,
                              translate("Two-Finger vertically"),
                              translate("Mouse Wheel")),
                          GestureInfo(
                              GestureIcons.icon_gesture_f_drag,
                              translate("Two-Finger Move"),
                              translate("Canvas Move")),
                          GestureInfo(
                              GestureIcons.icon_gesture_pinch,
                              translate("Pinch to Zoom"),
                              translate("Canvas Zoom")),
                        ]
                      : [
                          GestureInfo(
                              GestureIcons.icon_Mobile_Touch,
                              translate("One-Finger Tap"),
                              translate("Left Mouse")),
                          GestureInfo(
                              GestureIcons.icon_gesture_f_tap_,
                              translate("Two-Finger Tap"),
                              translate("Right Mouse")),
                          GestureInfo(
                              GestureIcons.icon_gesture_f_swipe_right,
                              translate("Double Tap & Move"),
                              translate("Mouse Drag")),
                          GestureInfo(
                              GestureIcons.icon_gesture_f_drag_up_down_,
                              translate("Two-Finger vertically"),
                              translate("Mouse Wheel")),
                          GestureInfo(
                              GestureIcons.icon_gesture_f_drag,
                              translate("Two-Finger Move"),
                              translate("Canvas Move")),
                          GestureInfo(
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
  const GestureInfo(this.icon, this.fromText, this.toText, {Key? key})
      : super(key: key);

  final String fromText;
  final String toText;
  final IconData icon;

  final textSize = 14.0;
  final textColor = MyTheme.accent80;
  final iconSize = 35.0;
  final iconColor = MyTheme.darkGray;

  @override
  Widget build(BuildContext context) {
    return Padding(
        padding: const EdgeInsets.symmetric(vertical: 5),
        child: Row(
          mainAxisAlignment: MainAxisAlignment.start,
          crossAxisAlignment: CrossAxisAlignment.center,
          children: [
            Padding(
                padding: const EdgeInsets.symmetric(horizontal: 0),
                child: Icon(
                  icon,
                  size: iconSize,
                  color: iconColor,
                )),
            Row(
              children: [
                Text(fromText,
                    style: TextStyle(fontSize: textSize, color: textColor)),
                Padding(
                    padding: const EdgeInsets.symmetric(horizontal: 5),
                    child: Icon(Icons.arrow_forward_rounded,
                        size: 20, color: iconColor)),
                Text(toText,
                    style: TextStyle(fontSize: textSize, color: textColor))
              ],
            )
          ],
        ));
  }
}
