import 'package:flutter/material.dart';

const sidebarColor = Color(0xFF0C6AF6);
const backgroundStartColor = Color(0xFF0583EA);
const backgroundEndColor = Color(0xFF0697EA);

class DesktopTitleBar extends StatelessWidget {
  final Widget? child;

  const DesktopTitleBar({Key? key, this.child}) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return Container(
      decoration: const BoxDecoration(
        gradient: LinearGradient(
            begin: Alignment.topCenter,
            end: Alignment.bottomCenter,
            colors: [backgroundStartColor, backgroundEndColor],
            stops: [0.0, 1.0]),
      ),
      child: Row(
        children: [
          Expanded(
            child: child ?? Offstage(),
          )
          // const WindowButtons()
        ],
      ),
    );
  }
}

// final buttonColors = WindowButtonColors(
//     iconNormal: const Color(0xFF805306),
//     mouseOver: const Color(0xFFF6A00C),
//     mouseDown: const Color(0xFF805306),
//     iconMouseOver: const Color(0xFF805306),
//     iconMouseDown: const Color(0xFFFFD500));
//
// final closeButtonColors = WindowButtonColors(
//     mouseOver: const Color(0xFFD32F2F),
//     mouseDown: const Color(0xFFB71C1C),
//     iconNormal: const Color(0xFF805306),
//     iconMouseOver: Colors.white);
//
// class WindowButtons extends StatelessWidget {
//   const WindowButtons({Key? key}) : super(key: key);
//
//   @override
//   Widget build(BuildContext context) {
//     return Row(
//       children: [
//         MinimizeWindowButton(colors: buttonColors, onPressed: () {
//           windowManager.minimize();
//         },),
//         MaximizeWindowButton(colors: buttonColors, onPressed: () async {
//           if (await windowManager.isMaximized()) {
//             windowManager.restore();
//           } else {
//             windowManager.maximize();
//           }
//         },),
//         CloseWindowButton(colors: closeButtonColors, onPressed: () {
//           windowManager.close();
//         },),
//       ],
//     );
//   }
// }
