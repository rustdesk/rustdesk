// Client — the assisted customer's screen.
//
// No way out to another computer, no editable permanent password, no
// unattended-access settings, no installation card. The customer sees the ID
// they read to support, the real availability of the service, and whether a
// session is running right now. Consent itself stays in the approval window
// the core opens, which requires a local click for every new session.

import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/common/formatter/id_formatter.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_hbb/models/state_model.dart';
import 'package:flutter_hbb/tefway/design.dart';
import 'package:flutter_hbb/tefway/notices.dart';
import 'package:flutter_hbb/tefway/strings.dart';
import 'package:flutter_hbb/tefway/widgets.dart';
import 'package:window_manager/window_manager.dart';

class TefwayClientHomePage extends StatefulWidget {
  const TefwayClientHomePage({super.key});

  @override
  State<TefwayClientHomePage> createState() => _TefwayClientHomePageState();
}

class _TefwayClientHomePageState extends State<TefwayClientHomePage> {
  static const _windowSize = Size(440, 720);

  Timer? _timer;
  SvcStatus _svcStatus = SvcStatus.notReady;
  int _connectionCount = 0;
  String _id = '';
  String _version = '';
  String _coreError = '';

  @override
  void initState() {
    super.initState();
    _id = gFFI.serverModel.serverId.text;
    _loadVersion();
    _timer = periodic_immediate(const Duration(seconds: 1), _refresh);
    WidgetsBinding.instance.addPostFrameCallback((_) async {
      try {
        await windowManager.setSize(_windowSize);
      } catch (e) {
        debugPrint('Tefway: failed to size the client window: $e');
      }
    });
  }

  @override
  void dispose() {
    _timer?.cancel();
    super.dispose();
  }

  Future<void> _loadVersion() async {
    try {
      final version = await bind.mainGetVersion();
      if (!mounted) return;
      setState(() => _version = version);
    } catch (e) {
      debugPrint('Tefway: failed to read app version: $e');
    }
  }

  Future<void> _refresh() async {
    await gFFI.serverModel.fetchID();
    var status = _svcStatus;
    var count = _connectionCount;
    try {
      final raw =
          jsonDecode(await bind.mainGetConnectStatus()) as Map<String, dynamic>;
      final statusNum = raw['status_num'] as int?;
      if (statusNum == null) {
        status = SvcStatus.notReady;
      } else if (statusNum == 0) {
        status = SvcStatus.connecting;
      } else if (statusNum == 1) {
        status = SvcStatus.ready;
      } else {
        status = SvcStatus.notReady;
      }
      count = (raw['video_conn_count'] as int?) ?? 0;
    } catch (e) {
      status = SvcStatus.notReady;
    }
    final coreError = await bind.mainGetError();
    if (!mounted) return;
    setState(() {
      _svcStatus = status;
      _connectionCount = count;
      _id = gFFI.serverModel.serverId.text;
      _coreError = coreError;
    });
  }

  void _copyId() {
    final raw = trimID(_id);
    if (raw.isEmpty) return;
    Clipboard.setData(ClipboardData(text: raw));
    showToast(TefwayStrings.copiedId);
  }

  Widget _availability() {
    if (_connectionCount > 0) {
      return TefwayStatusIndicator(
        status: TefwayStatus.attention,
        label: TefwayStrings.clientSessionActive,
        detail: TefwayStrings.clientSessionDetail(_connectionCount),
      );
    }
    switch (_svcStatus) {
      case SvcStatus.ready:
        return const TefwayStatusIndicator(
          status: TefwayStatus.positive,
          label: TefwayStrings.clientAwaiting,
          detail: TefwayStrings.clientAwaitingDetail,
        );
      case SvcStatus.connecting:
        return const TefwayStatusIndicator(
          status: TefwayStatus.busy,
          label: TefwayStrings.clientConnecting,
          detail: TefwayStrings.clientConnectingDetail,
        );
      case SvcStatus.notReady:
        return const TefwayStatusIndicator(
          status: TefwayStatus.critical,
          label: TefwayStrings.clientServerDown,
          detail: TefwayStrings.clientServerDownDetail,
        );
    }
  }

  @override
  Widget build(BuildContext context) {
    return TefwayAppShell(
      maxWidth: 420,
      padding: const EdgeInsets.all(TefwaySpacing.lg),
      child: TefwaySurfaceCard(
        padding: const EdgeInsets.all(TefwaySpacing.xl),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            const Center(child: TefwayBrandMark(size: 44)),
            const SizedBox(height: TefwaySpacing.md),
            const Text(
              TefwayStrings.productName,
              textAlign: TextAlign.center,
              style: TextStyle(
                fontSize: 20,
                fontWeight: FontWeight.w700,
                color: TefwayColors.textPrimary,
              ),
            ),
            const SizedBox(height: 2),
            const Text(
              TefwayStrings.clientSubtitle,
              textAlign: TextAlign.center,
              style: TefwayTextStyles.label,
            ),
            const SizedBox(height: TefwaySpacing.lg),
            const Divider(height: 1, color: TefwayColors.border),
            const SizedBox(height: TefwaySpacing.xl),
            TefwayDeviceIdCard(
              label: TefwayStrings.clientIdLabel,
              id: formatID(trimID(_id)),
              onCopy: _copyId,
            ),
            const SizedBox(height: TefwaySpacing.xl),
            Container(
              padding: const EdgeInsets.all(TefwaySpacing.md),
              decoration: BoxDecoration(
                color: TefwayColors.inputFill,
                borderRadius: TefwayRadius.mdAll,
                border: Border.all(color: TefwayColors.border, width: 1),
              ),
              child: _availability(),
            ),
            if (_connectionCount > 0) ...[
              const SizedBox(height: TefwaySpacing.sm),
              Text(TefwayStrings.clientSessionsHint,
                  style: TefwayTextStyles.caption),
            ],
            const SizedBox(height: TefwaySpacing.lg),
            const Text(
              TefwayStrings.clientGuidance,
              textAlign: TextAlign.center,
              style: TefwayTextStyles.body,
            ),
            const SizedBox(height: TefwaySpacing.sm),
            const Text(
              TefwayStrings.clientNoOutgoing,
              textAlign: TextAlign.center,
              style: TefwayTextStyles.caption,
            ),
            if (_coreError.isNotEmpty) ...[
              const SizedBox(height: TefwaySpacing.md),
              TefwayCoreErrorNotice(message: _coreError, onRetry: _refresh),
            ],
            ..._notices(),
            const SizedBox(height: TefwaySpacing.xl),
            TefwaySupportFooter(version: _version),
          ],
        ),
      ),
    );
  }

  List<Widget> _notices() {
    final notices = tefwayPortableNotices();
    if (notices.isEmpty) return const [];
    return [
      const SizedBox(height: TefwaySpacing.md),
      tefwayNoticeColumn(notices),
    ];
  }
}
