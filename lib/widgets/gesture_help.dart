import 'package:flutter/material.dart';
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
}

class GestureHelp extends StatefulWidget {
  GestureHelp({Key? key}) : super(key: key);

  @override
  State<StatefulWidget> createState() => _GestureHelpState();
}

class _GestureHelpState extends State<GestureHelp> {
  var _selectedIndex = 0;
  var _touchMode = false;

  @override
  Widget build(BuildContext context) {
    return Center(
        child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 30, vertical: 10),
            child: Column(
              mainAxisAlignment: MainAxisAlignment.center,
              crossAxisAlignment: CrossAxisAlignment.center,
              children: <Widget>[
                ToggleSwitch(
                  initialLabelIndex: _selectedIndex,
                  totalSwitches: 2,
                  minWidth: 130,
                  fontSize: 15,
                  iconSize: 20,
                  labels: ["触摸板模式", "触屏模式"],
                  icons: [
                    GestureIcons.icon_mouse,
                    GestureIcons.icon_Tablet_Touch
                  ],
                  onToggle: (index) {
                    debugPrint(index.toString());
                    setState(() {
                      _touchMode = index == 0 ? false : true;
                      _selectedIndex = index ?? 0;
                    });
                  },
                ),
                const SizedBox(height: 15),
                Column(
                  mainAxisAlignment: MainAxisAlignment.start,
                  crossAxisAlignment: CrossAxisAlignment.center,
                  children: _touchMode
                      ? const [
                          GestureInfo(
                              GestureIcons.icon_Mobile_Touch, "单指轻触", "点击对应位置"),
                          GestureInfo(GestureIcons.icon_gesture_press_hold,
                              "单指长按", "鼠标右键"),
                          GestureInfo(GestureIcons.icon_gesture_f_swipe_right,
                              "单指移动", "鼠标选中拖动"),
                          GestureInfo(GestureIcons.icon_gesture_f_drag_up_down_,
                              "双指垂直滑动", "鼠标滚轮"),
                          GestureInfo(
                              GestureIcons.icon_gesture_f_drag, "双指移动", "移动画布"),
                          GestureInfo(
                              GestureIcons.icon_gesture_pinch, "双指缩放", "缩放画布"),
                        ]
                      : const [
                          GestureInfo(
                              GestureIcons.icon_gesture_tap, "单指轻触", "鼠标左键"),
                          GestureInfo(
                              GestureIcons.icon_gesture_f_tap_, "双指轻触", "鼠标右键"),
                          GestureInfo(GestureIcons.icon_gesture_f_swipe_right,
                              "双击并移动", "鼠标选中拖动"),
                          GestureInfo(GestureIcons.icon_gesture_f_drag_up_down_,
                              "双指垂直滑动", "鼠标滚轮"),
                          GestureInfo(
                              GestureIcons.icon_gesture_f_drag, "双指移动", "移动画布"),
                          GestureInfo(
                              GestureIcons.icon_gesture_pinch, "双指缩放", "缩放画布"),
                        ],
                ),
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

  final textSize = 15.0;
  final textColor = Colors.blue;
  final iconSize = 35.0;
  final iconColor = Colors.black54;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
        width: 280,
        child: Padding(
            padding: const EdgeInsets.symmetric(vertical: 5),
            child: Row(
              mainAxisAlignment: MainAxisAlignment.start,
              crossAxisAlignment: CrossAxisAlignment.center,
              children: [
                Padding(
                    padding: const EdgeInsets.symmetric(horizontal: 15),
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
            )));
  }
}
