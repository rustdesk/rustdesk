import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';

/// Default "OK" button with "check" icon
Widget dialogSubmitButton({String? text, required VoidCallback? onPressed}) {
  return dialogButton(
    text ?? 'OK',
    icon: Icon(Icons.done_rounded, size: 16),
    onPressed: onPressed,
    buttonStyle: ElevatedButton.styleFrom(
      padding: EdgeInsets.fromLTRB(14, 14, 18, 14),
    ),
    style: TextStyle(fontSize: 15, fontWeight: FontWeight.normal),
  );
}

/// Default "cancel" button with "cancel" icon
Widget dialogCancelButton({String? text, required VoidCallback? onPressed}) {
  return dialogButton(
    text ?? 'Cancel',
    isOutline: true,
    icon: Icon(Icons.close_outlined, size: 16),
    onPressed: onPressed,
    buttonStyle: ElevatedButton.styleFrom(
      padding: EdgeInsets.fromLTRB(14, 15, 18, 15),
    ),
    style: TextStyle(fontSize: 15, fontWeight: FontWeight.normal),
  );
}
