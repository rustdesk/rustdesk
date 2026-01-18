import 'package:flutter/material.dart';
import 'package:flutter_hbb/common/remote_input_event_log.dart';
import 'package:flutter_hbb/models/model.dart';

class RemoteInputLogOverlay extends StatefulWidget {
  const RemoteInputLogOverlay({
    super.key,
    required this.cursorModel,
  });

  final CursorModel cursorModel;

  @override
  State<RemoteInputLogOverlay> createState() => _RemoteInputLogOverlayState();
}

class _RemoteInputLogOverlayState extends State<RemoteInputLogOverlay> {
  static const double _left = 10;
  static const double _top = 10;
  static const double _width = 320;
  static const double _heightExpanded = 220;
  static const double _heightCollapsed = 44;

  bool _collapsed = false;
  Rect? _blockedRect;

  void _setBlockedRect({required bool enabled}) {
    final newRect = enabled
        ? Rect.fromLTWH(
            _left,
            _top,
            _width,
            _collapsed ? _heightCollapsed : _heightExpanded,
          )
        : null;

    if (_blockedRect != null) {
      widget.cursorModel.removeBlockedRect(_blockedRect!);
      _blockedRect = null;
    }
    if (newRect != null) {
      widget.cursorModel.addBlockedRect(newRect);
      _blockedRect = newRect;
    }
  }

  @override
  void dispose() {
    _setBlockedRect(enabled: false);
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final enabled = RemoteInputEventLog.isEnabled;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) _setBlockedRect(enabled: enabled);
    });

    if (!enabled) return const Offstage();

    return Positioned(
      left: _left,
      top: _top,
      width: _width,
      height: _collapsed ? _heightCollapsed : _heightExpanded,
      child: Semantics(
        label: 'u2_remote_input_log_overlay',
        container: true,
        child: Material(
          color: Colors.transparent,
          child: Container(
            decoration: BoxDecoration(
              color: const Color(0xCC000000),
              borderRadius: BorderRadius.circular(12),
              border: Border.all(color: Colors.white24),
            ),
            child: Column(
              children: [
                SizedBox(
                  height: _heightCollapsed,
                  child: Row(
                    children: [
                      const SizedBox(width: 12),
                      const Text(
                        'E2E LOG',
                        style: TextStyle(
                          color: Colors.white,
                          fontSize: 12,
                          fontWeight: FontWeight.w600,
                        ),
                      ),
                      const Spacer(),
                      Semantics(
                        label: 'u2_remote_input_log_clear',
                        button: true,
                        child: TextButton(
                          onPressed: RemoteInputEventLog.clear,
                          child: const Text(
                            '清空',
                            style: TextStyle(color: Colors.white),
                          ),
                        ),
                      ),
                      Semantics(
                        label: 'u2_remote_input_log_toggle',
                        button: true,
                        child: IconButton(
                          onPressed: () =>
                              setState(() => _collapsed = !_collapsed),
                          icon: Icon(
                            _collapsed
                                ? Icons.keyboard_arrow_down
                                : Icons.keyboard_arrow_up,
                            color: Colors.white,
                          ),
                        ),
                      ),
                    ],
                  ),
                ),
                if (!_collapsed)
                  Expanded(
                    child: Padding(
                      padding: const EdgeInsets.fromLTRB(12, 0, 12, 12),
                      child: ValueListenableBuilder<int>(
                        valueListenable: RemoteInputEventLog.revision,
                        builder: (context, _, __) {
                          return SingleChildScrollView(
                            child: Semantics(
                              label: 'u2_remote_input_log_text',
                              child: Text(
                                RemoteInputEventLog.dumpText(lastN: 40),
                                style: const TextStyle(
                                  color: Colors.white,
                                  fontSize: 11,
                                  height: 1.25,
                                ),
                              ),
                            ),
                          );
                        },
                      ),
                    ),
                  ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
