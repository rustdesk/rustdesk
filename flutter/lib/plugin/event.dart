void handlePluginEvent(
  Map<String, dynamic> evt,
  String peer,
  Function(Map<String, dynamic> e) handleMsgBox,
) {
  if (evt['content']?['c'] == null) return;
  final t = evt['content']?['t'];
  if (t == 'MsgBox') {
    handleMsgBox(evt['content']?['c']);
  }
}
