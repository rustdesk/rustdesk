void handlePluginEvent(
  Map<String, dynamic> evt,
  String peer,
  Function(Map<String, dynamic> e) handleMsgBox,
) {
  // content
  //
  // {
  //   "t": "Option",
  //   "c": {
  //     "id": "id from RustDesk platform",
  //     "name": "Privacy Mode",
  //     "version": "v0.1.0",
  //     "location": "client|remote|toolbar|display",
  //     "key": "privacy-mode",
  //     "value": "1"
  //   }
  // }
  //
  // {
  //   "t": "MsgBox",
  //   "c": {
  //     "type": "custom-nocancel",
  //     "title": "Privacy Mode",
  //     "text": "Failed unknown",
  //     "link": ""
  //   }
  // }
  //
  if (evt['content']?['c'] == null) return;
  final t = evt['content']?['t'];
  if (t == 'Option') {
    handleOptionEvent(evt['content']?['c'], peer);
  } else if (t == 'MsgBox') {
    handleMsgBox(evt['content']?['c']);
  }
}

void handleOptionEvent(Map<String, dynamic> evt, String peer) {
  // content
  //
  // {
  //   "id": "id from RustDesk platform",
  //   "name": "Privacy Mode",
  //   "version": "v0.1.0",
  //   "location": "client|remote|toolbar|display",
  //   "key": "privacy-mode",
  //   "value": "1"
  // }
  //
  final key = evt['key'];
  final value = evt['value'];
  if (key == 'privacy-mode') {
    if (value == '1') {
      // enable privacy mode
    } else {
      // disable privacy mode
    }
  }
}
