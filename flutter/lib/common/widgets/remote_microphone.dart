import 'package:flutter/material.dart';
import 'package:get/get.dart';

import '../../common.dart';
import '../../models/model.dart';
import '../../models/platform_model.dart';
import '../../models/remote_microphone_model.dart';

class RemoteMicrophoneButton extends StatelessWidget {
  const RemoteMicrophoneButton({
    super.key,
    required this.ffi,
    required this.peerId,
  });
  final FFI ffi;
  final String peerId;
  @override
  Widget build(BuildContext context) {
    final mic = ffi.chatModel.remoteMicrophone;
    return Obx(
      () => Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          IconButton(
            tooltip: mic.error.value.isEmpty
                ? '${translate('Remote microphone')}: ${translate(mic.state.value)}'
                : mic.error.value,
            color:
                mic.state.value == 'active' ? Colors.greenAccent : Colors.white,
            icon: Icon(mic.state.value == 'active' ? Icons.mic : Icons.mic_off),
            onPressed: mic.state.value == 'unavailable' ||
                    mic.state.value == 'starting'
                ? null
                : () {
                    if (mic.state.value == 'active') {
                      mic.stop(mute: true);
                    } else {
                      mic.start();
                    }
                  },
          ),
          PopupMenuButton<String>(
            icon: const Icon(Icons.arrow_drop_down, color: Colors.white),
            onSelected: (value) {
              if (value == 'stop') {
                mic.stop();
              }
              if (value == 'auto') {
                final old = bind.mainGetPeerOptionSync(
                      id: peerId,
                      key: RemoteMicrophoneModel.preference,
                    ) ==
                    'Y';
                bind.mainSetPeerOption(
                  id: peerId,
                  key: RemoteMicrophoneModel.preference,
                  value: old ? '' : 'Y',
                );
              }
            },
            itemBuilder: (_) => [
              PopupMenuItem(
                value: 'stop',
                child: Text(translate('Stop remote microphone')),
              ),
              CheckedPopupMenuItem(
                value: 'auto',
                checked: bind.mainGetPeerOptionSync(
                      id: peerId,
                      key: RemoteMicrophoneModel.preference,
                    ) ==
                    'Y',
                child: Text(
                    translate('Start microphone automatically for this PC')),
              ),
            ],
          ),
        ],
      ),
    );
  }
}
