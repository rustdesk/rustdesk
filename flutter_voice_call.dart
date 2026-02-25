// Voice Calling UI Integration for RustDesk Flutter Client
//
// This file demonstrates how to integrate voice calling into the RustDesk Flutter UI.
// File location: flutter/lib/models/voice_call.dart

import 'package:flutter/material.dart';
import 'package:get/get.dart';
import '../common.dart';
import 'model.dart';

/// Controller for voice call state and actions
class VoiceCallController extends GetxController {
  /// Is voice call currently active
  final isActive = false.obs;

  /// Is incoming call waiting for response
  final incomingCall = false.obs;

  /// Is local audio muted
  final isMuted = false.obs;

  /// Call duration in seconds
  final callDuration = 0.obs;

  /// Estimated network quality (0-100)
  final networkQuality = 100.obs;

  /// Current bandwidth (kbps)
  final bandwidth = 0.obs;

  /// Jitter buffer delay (ms)
  final jitterDelay = 0.obs;

  /// Session model
  late SessionModel session;

  /// Timer for call duration
  Duration? _durationTimer;

  @override
  void onInit() {
    super.onInit();
    session = Get.find<SessionModel>();
  }

  /// Request to start a voice call
  void requestVoiceCall() async {
    try {
      bind.sessionRequestVoiceCall(sessionId: session.sessionId);
      isActive.value = true;
      incomingCall.value = false;
      _startCallTimer();
    } catch (e) {
      Get.snackbar('Error', 'Failed to start voice call: $e');
    }
  }

  /// Accept incoming voice call
  void acceptVoiceCall() {
    try {
      bind.sessionAcceptVoiceCall(sessionId: session.sessionId);
      isActive.value = true;
      incomingCall.value = false;
      _startCallTimer();
    } catch (e) {
      Get.snackbar('Error', 'Failed to accept call: $e');
    }
  }

  /// Reject incoming voice call
  void rejectVoiceCall() {
    try {
      bind.sessionRejectVoiceCall(sessionId: session.sessionId);
      incomingCall.value = false;
    } catch (e) {
      Get.snackbar('Error', 'Failed to reject call: $e');
    }
  }

  /// End active voice call
  void endVoiceCall() {
    try {
      bind.sessionEndVoiceCall(sessionId: session.sessionId);
      isActive.value = false;
      incomingCall.value = false;
      callDuration.value = 0;
      _durationTimer = null;
    } catch (e) {
      Get.snackbar('Error', 'Failed to end call: $e');
    }
  }

  /// Toggle audio mute
  void toggleMute() {
    try {
      isMuted.value = !isMuted.value;
      bind.sessionMuteVoiceCall(
        sessionId: session.sessionId,
        isMuted: isMuted.value,
      );
    } catch (e) {
      Get.snackbar('Error', 'Failed to toggle mute: $e');
    }
  }

  /// Update call statistics from Rust backend
  void updateCallStats(
    int bandwidth,
    int jitterMs,
    int quality,
  ) {
    this.bandwidth.value = bandwidth;
    jitterDelay.value = jitterMs;
    networkQuality.value = quality;
  }

  /// Start timer for call duration
  void _startCallTimer() {
    _durationTimer = Duration(seconds: 1);
    Future.delayed(Duration(seconds: 1), () {
      if (isActive.value) {
        callDuration.value++;
        _startCallTimer();
      }
    });
  }

  /// Format duration as MM:SS
  String formatDuration(int seconds) {
    final minutes = seconds ~/ 60;
    final secs = seconds % 60;
    return '${minutes.toString().padLeft(2, '0')}:${secs.toString().padLeft(2, '0')}';
  }

  @override
  void onClose() {
    if (isActive.value) {
      endVoiceCall();
    }
    super.onClose();
  }
}

/// Voice call overlay widget
class VoiceCallOverlay extends StatelessWidget {
  final SessionModel session;

  const VoiceCallOverlay({required this.session});

  @override
  Widget build(BuildContext context) {
    return GetBuilder<VoiceCallController>(
      init: VoiceCallController(),
      builder: (controller) {
        controller.session = session;

        return Obx(
          () => controller.isActive.value
              ? _buildActiveCallUI(controller, context)
              : controller.incomingCall.value
                  ? _buildIncomingCallUI(controller, context)
                  : SizedBox.shrink(),
        );
      },
    );
  }

  /// Build UI for active call
  Widget _buildActiveCallUI(
    VoiceCallController controller,
    BuildContext context,
  ) {
    return Positioned(
      top: MediaQuery.of(context).padding.top + 10,
      left: 10,
      right: 10,
      child: Container(
        decoration: BoxDecoration(
          color: Colors.grey[900],
          borderRadius: BorderRadius.circular(12),
          boxShadow: [
            BoxShadow(
              color: Colors.black.withOpacity(0.3),
              blurRadius: 8,
            ),
          ],
        ),
        padding: EdgeInsets.all(12),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            // Header with duration
            Row(
              mainAxisAlignment: MainAxisAlignment.spaceBetween,
              children: [
                Text(
                  '🎤 Voice Call',
                  style: TextStyle(
                    color: Colors.white,
                    fontSize: 16,
                    fontWeight: FontWeight.bold,
                  ),
                ),
                Obx(
                  () => Text(
                    controller.formatDuration(controller.callDuration.value),
                    style: TextStyle(
                      color: Colors.grey[300],
                      fontSize: 14,
                    ),
                  ),
                ),
              ],
            ),
            SizedBox(height: 8),

            // Network quality indicator
            Obx(() {
              final quality = controller.networkQuality.value;
              final qualityColor = quality > 80
                  ? Colors.green
                  : quality > 50
                      ? Colors.yellow
                      : Colors.red;

              return Row(
                children: [
                  Container(
                    width: 8,
                    height: 8,
                    decoration: BoxDecoration(
                      color: qualityColor,
                      shape: BoxShape.circle,
                    ),
                  ),
                  SizedBox(width: 8),
                  Expanded(
                    child: ClipRRect(
                      borderRadius: BorderRadius.circular(4),
                      child: LinearProgressIndicator(
                        value: quality / 100,
                        minHeight: 4,
                        backgroundColor: Colors.grey[700],
                        valueColor: AlwaysStoppedAnimation(qualityColor),
                      ),
                    ),
                  ),
                  SizedBox(width: 8),
                  Text(
                    '${quality}%',
                    style: TextStyle(
                      color: Colors.grey[300],
                      fontSize: 12,
                    ),
                  ),
                ],
              );
            }),

            SizedBox(height: 8),

            // Network stats
            Obx(
              () => Text(
                '${controller.bandwidth.value} kbps • Jitter: ${controller.jitterDelay.value}ms',
                style: TextStyle(
                  color: Colors.grey[400],
                  fontSize: 11,
                ),
                textAlign: TextAlign.center,
              ),
            ),

            SizedBox(height: 12),

            // Control buttons
            Row(
              mainAxisAlignment: MainAxisAlignment.spaceBetween,
              children: [
                // Mute button
                Obx(
                  () => FloatingActionButton(
                    mini: true,
                    backgroundColor: controller.isMuted.value
                        ? Colors.red
                        : Colors.blue,
                    onPressed: () => controller.toggleMute(),
                    child: Icon(
                      controller.isMuted.value
                          ? Icons.mic_off
                          : Icons.mic,
                    ),
                  ),
                ),

                // End call button
                FloatingActionButton(
                  mini: true,
                  backgroundColor: Colors.red,
                  onPressed: () => controller.endVoiceCall(),
                  child: Icon(Icons.call_end),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }

  /// Build UI for incoming call
  Widget _buildIncomingCallUI(
    VoiceCallController controller,
    BuildContext context,
  ) {
    return Positioned(
      top: MediaQuery.of(context).size.height / 2 - 100,
      left: 20,
      right: 20,
      child: Dialog(
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(16)),
        child: Container(
          padding: EdgeInsets.all(20),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(
                Icons.phone_incoming,
                size: 48,
                color: Colors.blue,
              ),
              SizedBox(height: 16),
              Text(
                'Incoming Voice Call',
                style: TextStyle(
                  fontSize: 20,
                  fontWeight: FontWeight.bold,
                ),
              ),
              SizedBox(height: 8),
              Text(
                'from ${session.ffiModel.pi.username}',
                style: TextStyle(
                  fontSize: 14,
                  color: Colors.grey[600],
                ),
              ),
              SizedBox(height: 24),
              Row(
                mainAxisAlignment: MainAxisAlignment.spaceEvenly,
                children: [
                  // Decline button
                  ElevatedButton.icon(
                    onPressed: () => controller.rejectVoiceCall(),
                    icon: Icon(Icons.call_end),
                    label: Text('Decline'),
                    style: ElevatedButton.styleFrom(
                      backgroundColor: Colors.red,
                    ),
                  ),

                  // Accept button
                  ElevatedButton.icon(
                    onPressed: () => controller.acceptVoiceCall(),
                    icon: Icon(Icons.call),
                    label: Text('Accept'),
                    style: ElevatedButton.styleFrom(
                      backgroundColor: Colors.green,
                    ),
                  ),
                ],
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// Voice call button for session toolbar
class VoiceCallButton extends StatelessWidget {
  final SessionModel session;

  const VoiceCallButton({required this.session});

  @override
  Widget build(BuildContext context) {
    return GetBuilder<VoiceCallController>(
      init: VoiceCallController(),
      builder: (controller) {
        controller.session = session;

        return Obx(
          () => IconButton(
            icon: Icon(
              controller.isActive.value
                  ? Icons.phone
                  : Icons.phone_outlined,
            ),
            onPressed: controller.isActive.value
                ? () => controller.endVoiceCall()
                : () => controller.requestVoiceCall(),
            color: controller.isActive.value ? Colors.green : Colors.grey,
            tooltip: controller.isActive.value
                ? 'End Voice Call'
                : 'Start Voice Call',
          ),
        );
      },
    );
  }
}

/// Voice call device selection dialog
class VoiceCallDeviceSelector extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Dialog(
      child: SingleChildScrollView(
        child: Padding(
          padding: EdgeInsets.all(16),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              Text(
                'Audio Devices',
                style: TextStyle(
                  fontSize: 18,
                  fontWeight: FontWeight.bold,
                ),
              ),
              SizedBox(height: 16),

              // Microphone selection
              Text(
                'Microphone',
                style: TextStyle(fontWeight: FontWeight.bold),
              ),
              SizedBox(height: 8),
              _buildDeviceList('input'),
              SizedBox(height: 16),

              // Speaker selection
              Text(
                'Speaker',
                style: TextStyle(fontWeight: FontWeight.bold),
              ),
              SizedBox(height: 8),
              _buildDeviceList('output'),
              SizedBox(height: 16),

              // Close button
              ElevatedButton(
                onPressed: () => Navigator.pop(context),
                child: Text('Done'),
              ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _buildDeviceList(String type) {
    // This would typically fetch device list from Rust backend
    return Container(
      decoration: BoxDecoration(
        border: Border.all(color: Colors.grey),
        borderRadius: BorderRadius.circular(8),
      ),
      child: ListView(
        shrinkWrap: true,
        children: [
          _buildDeviceItem('Default Device', true),
          _buildDeviceItem('Headphones', false),
          _buildDeviceItem('Bluetooth Headset', false),
        ],
      ),
    );
  }

  Widget _buildDeviceItem(String name, bool isSelected) {
    return ListTile(
      title: Text(name),
      trailing: isSelected
          ? Icon(Icons.check, color: Colors.blue)
          : null,
      onTap: () {
        // Handle device selection
      },
    );
  }
}

// Extension on SessionModel to integrate voice calling
extension VoiceCallBinding on SessionModel {
  /// Check if voice calling is supported
  bool get supportsVoiceCall {
    // Check peer version or feature flags
    return true;
  }

  /// Show incoming call notification
  void showIncomingCallNotification() {
    final controller = Get.find<VoiceCallController>();
    controller.incomingCall.value = true;
  }

  /// Callback from Rust when call state changes
  void onVoiceCallStateChanged(String state) {
    final controller = Get.find<VoiceCallController>();

    switch (state) {
      case 'started':
        controller.isActive.value = true;
        break;
      case 'ended':
        controller.endVoiceCall();
        break;
      case 'incoming':
        showIncomingCallNotification();
        break;
    }
  }

  /// Callback from Rust for audio statistics
  void onAudioStatsUpdated({
    required int bandwidth,
    required int jitterMs,
    required int quality,
  }) {
    final controller = Get.find<VoiceCallController>();
    controller.updateCallStats(bandwidth, jitterMs, quality);
  }
}
