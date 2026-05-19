// ⚠️ PARKED — do not wire this back into the UI without reading this comment.
//
// This sheet was the in-session "Send File" UI launched from the PowerStrip.
// It never worked end-to-end on any peer.
//
// Root cause is architectural, not in this file. RustDesk's server requires
// a login_request with Union::FileTransfer to set self.file_transfer = Some
// (see src/server/connection.rs handle_login_request_without_validation).
// Without that, file-send messages arrive at the host but no worker is
// dispatched to handle them — the call returns cleanly on the client side
// while the host silently drops the request. No progress events ever fire.
//
// Tabby remote-desktop sessions are opened with isFileTransfer=false, so
// every sessionSendFiles call from inside the active session falls into
// this dead zone. The destination-picker logic, the homePath bypass, and
// the in-app debug log here are all correct — the protocol just doesn't
// route to us.
//
// The PowerStrip "send file" button now navigates to the standard
// FileManagerPage instead, which opens a dedicated isFileTransfer session
// via gFFI.start(). That tears down the remote-desktop view (gFFI is a
// singleton), but it's the only way to actually transfer a file today.
//
// To revive this sheet you'd need either:
//   1. A parallel FFI instance with its own sessionId and per-session
//      event routing at the platform layer (native_model.dart) — non-trivial.
//   2. An upstream RustDesk protocol change to allow file-transfer on a
//      defaultConn session — outside Tabby's control.
import 'dart:io';
import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/models/file_model.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:get/get.dart';
import 'package:path/path.dart' as p;

// Internal state machine:
// destinationPicker → sending → done
//                       ↑
//                 (send more files resets to destinationPicker)

enum _SheetState { destinationPicker, sending, done }

class FileSendSheet extends StatefulWidget {
  final FFI ffi;

  const FileSendSheet({super.key, required this.ffi});

  @override
  State<FileSendSheet> createState() => _FileSendSheetState();
}

class _FileSendSheetState extends State<FileSendSheet> {
  _SheetState _sheetState = _SheetState.destinationPicker;

  // Destination picker state
  String? _selectedChip = '~';
  final _pathController = TextEditingController(text: '~');

  // Sending / done state
  List<PlatformFile> _files = [];
  List<int> _jobIds = [];
  String _destination = '~';
  bool _accordionExpanded = false;

  // In-app debug log — accumulates step-by-step events so we can diagnose
  // TestFlight builds without console access.
  final List<String> _debugLog = [];
  bool _debugExpanded = false;

  void _log(String msg) {
    final ts = DateTime.now();
    final stamp =
        '${ts.minute.toString().padLeft(2, '0')}:${ts.second.toString().padLeft(2, '0')}.${ts.millisecond.toString().padLeft(3, '0')}';
    debugPrint('[FileSend] $msg');
    if (!mounted) {
      _debugLog.add('$stamp  $msg');
      return;
    }
    setState(() => _debugLog.add('$stamp  $msg'));
  }

  static const _chips = [
    ('🏠', 'Home', '~'),
    ('🖥', 'Desktop', '~/Desktop'),
    ('⬇️', 'Downloads', '~/Downloads'),
    ('📄', 'Documents', '~/Documents'),
    ('🗂', '/tmp', '/tmp'),
  ];

  @override
  void initState() {
    super.initState();
    // In Tabby's remote-desktop session, fileModel.onReady() is gated on
    // ConnType.fileTransfer and never runs, so the remote home path stays
    // empty. Kick off a remote directory listing now — the response flows
    // through receiveFileDir → initDirAndHome and populates homePath while
    // the user picks a destination.
    final remoteCtrl = widget.ffi.fileModel.remoteController;
    _log('initState: remoteHome="${remoteCtrl.homePath}"');
    if (remoteCtrl.homePath.isEmpty) {
      _log('priming remote home via openDirectory("")');
      remoteCtrl.openDirectory('');
    }
  }

  @override
  void dispose() {
    _pathController.dispose();
    super.dispose();
  }

  // ── Picker entry point ───────────────────────────────────────────────────

  Future<void> _pickAndSend() async {
    _log('pickFiles: starting (dest="${_pathController.text}")');
    FilePickerResult? result;
    try {
      result = await FilePicker.platform.pickFiles(allowMultiple: true);
    } catch (e, st) {
      _log('pickFiles threw: $e');
      debugPrint('[FileSend] stack: $st');
      if (mounted) {
        _showErrorDialog(
          'Couldn\'t open file picker',
          'iOS returned an error while opening the file browser.\n\n$e',
        );
      }
      return;
    }

    if (result == null) {
      _log('pickFiles: user cancelled');
      return;
    }

    _log('pickFiles returned ${result.files.length} file(s)');
    for (final f in result.files) {
      _log('  name=${f.name} size=${f.size} path=${f.path != null ? "ok" : "NULL"}');
    }

    if (result.files.isEmpty) {
      if (mounted) {
        _showErrorDialog(
          'No files selected',
          'iOS returned an empty selection. This usually means the file '
              'couldn\'t be copied out of its source app (Files, iCloud Drive, '
              'or a third-party provider). Try copying the file to On My '
              'iPhone → Tabby first, then picking it again.',
        );
      }
      return;
    }

    final invalid = result.files.where((f) => f.path == null).toList();
    if (invalid.isNotEmpty) {
      _log('${invalid.length} file(s) had null path; aborting');
      if (mounted) {
        _showErrorDialog(
          'Selected file is unavailable',
          'iOS didn\'t provide a local path for: '
              '${invalid.map((f) => f.name).join(", ")}\n\n'
              'The file may be in iCloud and not yet downloaded, or stored '
              'in a provider that doesn\'t expose a path. Open it in the '
              'Files app first to download, then try again.',
        );
      }
      return;
    }

    await _startTransfer(result.files);
  }

  void _showErrorDialog(String title, String message) {
    showDialog<void>(
      context: context,
      builder: (ctx) => AlertDialog(
        backgroundColor: const Color(0xFF1E293B),
        title: Text(
          title,
          style: const TextStyle(
            color: Color(0xFFE2E8F0),
            fontSize: 16,
            fontWeight: FontWeight.w700,
          ),
        ),
        content: Text(
          message,
          style: const TextStyle(color: Color(0xFFE2E8F0), fontSize: 13),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(ctx).pop(),
            child: const Text(
              'OK',
              style: TextStyle(color: Color(0xFF2563EB)),
            ),
          ),
        ],
      ),
    );
  }

  // ── Transfer initiation ──────────────────────────────────────────────────

  Future<void> _startTransfer(List<PlatformFile> files) async {
    final remoteCtrl = widget.ffi.fileModel.remoteController;
    final isRemoteWindows = remoteCtrl.options.value.isWindows;
    final rawDest = _pathController.text.trim();
    _log('_startTransfer: ${files.length} file(s), '
        'rawDest="$rawDest", isRemoteWindows=$isRemoteWindows');

    // Only the ~-prefixed destinations require remoteHome. Absolute paths
    // like /tmp send straight through.
    String destDir;
    if (rawDest.startsWith('~')) {
      var remoteHome = remoteCtrl.homePath;
      _log('~ destination; remoteHome="$remoteHome"');

      // If the prime kicked off in initState hasn't returned yet, retry it and
      // poll briefly. fetchDirectory has its own 2s internal timeout.
      if (remoteHome.isEmpty) {
        _log('remoteHome empty; re-priming and waiting up to 3s');
        remoteCtrl.openDirectory('');
        final deadline =
            DateTime.now().add(const Duration(milliseconds: 3000));
        while (
            DateTime.now().isBefore(deadline) && remoteCtrl.homePath.isEmpty) {
          await Future.delayed(const Duration(milliseconds: 100));
        }
        remoteHome = remoteCtrl.homePath;
        _log('after wait: remoteHome="$remoteHome"');
      }

      if (remoteHome.isEmpty) {
        _log('aborting: remoteHome still empty');
        if (mounted) {
          _showErrorDialog(
            'Remote home directory unavailable',
            'Couldn\'t resolve the remote machine\'s home directory. Try '
                'sending to an absolute path like /tmp, or try again in a '
                'moment.',
          );
        }
        return;
      }

      destDir = rawDest.replaceFirst('~', remoteHome);
    } else {
      destDir = rawDest;
      _log('absolute destination; skipping remoteHome lookup');
    }

    _log('resolved destDir="$destDir"');

    final ids = <int>[];
    for (final file in files) {
      if (file.path == null) continue;
      final entry = Entry();
      entry.path = file.path!;
      entry.name = p.basename(file.path!);
      entry.size = await File(file.path!).length();
      // entryType >= 4 → isFile == true (getter: entryType > 3)
      entry.entryType = 4;

      final jobId =
          widget.ffi.fileModel.jobController.addTransferJob(entry, false);
      ids.add(jobId);

      final to = PathUtil.join(destDir, entry.name, isRemoteWindows);
      _log('sessionSendFiles jobId=$jobId to="$to" size=${entry.size}');
      await bind.sessionSendFiles(
        sessionId: widget.ffi.sessionId,
        actId: jobId,
        path: file.path!,
        to: to,
        fileNum: 0,
        includeHidden: false,
        isRemote: false,
        isDir: false,
      );
    }

    _log('all sessionSendFiles dispatched; entering sending state');
    setState(() {
      _files = files;
      _jobIds = ids;
      _destination = destDir;
      _sheetState = _SheetState.sending;
      _accordionExpanded = false;
    });
  }

  void _cancel() {
    for (final id in _jobIds) {
      widget.ffi.fileModel.jobController.cancelJob(id);
    }
    Navigator.of(context).pop();
  }

  Future<void> _retryFailed(List<JobProgress> jobs) async {
    final failedFiles = <PlatformFile>[];
    for (var i = 0; i < _jobIds.length; i++) {
      final job = jobs.firstWhereOrNull((j) => j.id == _jobIds[i]);
      if (job != null && job.state == JobState.error && i < _files.length) {
        failedFiles.add(_files[i]);
      }
    }
    if (failedFiles.isNotEmpty) {
      await _startTransfer(failedFiles);
    }
  }

  void _checkAllDone(List<JobProgress> jobs) {
    if (_jobIds.isEmpty) return;
    final allSettled = _jobIds.every((id) {
      final job = jobs.firstWhereOrNull((j) => j.id == id);
      if (job == null) return false;
      return job.state == JobState.done || job.state == JobState.error;
    });
    if (allSettled && _sheetState == _SheetState.sending) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) setState(() => _sheetState = _SheetState.done);
      });
    }
  }

  // ── Formatting helpers ───────────────────────────────────────────────────

  String _formatBytes(int bytes) {
    if (bytes < 1024) return '$bytes B';
    if (bytes < 1024 * 1024) return '${(bytes / 1024).toStringAsFixed(1)} KB';
    return '${(bytes / (1024 * 1024)).toStringAsFixed(1)} MB';
  }

  String _fileIcon(String path) {
    final ext = p.extension(path).toLowerCase();
    if (['.jpg', '.jpeg', '.png', '.gif', '.webp', '.heic'].contains(ext)) {
      return '🖼';
    }
    if (['.mp4', '.mov', '.avi', '.mkv'].contains(ext)) return '🎬';
    if (['.mp3', '.aac', '.wav', '.flac'].contains(ext)) return '🎵';
    if (['.zip', '.tar', '.gz', '.rar', '.7z'].contains(ext)) return '📦';
    if (['.pdf'].contains(ext)) return '📕';
    return '📄';
  }

  // ── Sheet handle ─────────────────────────────────────────────────────────

  Widget _buildHandle() {
    return Center(
      child: Container(
        width: 36,
        height: 4,
        margin: const EdgeInsets.only(bottom: 16),
        decoration: BoxDecoration(
          color: Colors.grey[600],
          borderRadius: BorderRadius.circular(2),
        ),
      ),
    );
  }

  Widget _buildDebugLog() {
    if (_debugLog.isEmpty) return const SizedBox.shrink();
    return Padding(
      padding: const EdgeInsets.only(top: 12),
      child: Container(
        decoration: BoxDecoration(
          color: const Color(0xFF0F172A),
          borderRadius: BorderRadius.circular(8),
          border: Border.all(color: const Color(0xFF334155), width: 1),
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            GestureDetector(
              onTap: () =>
                  setState(() => _debugExpanded = !_debugExpanded),
              child: Padding(
                padding: const EdgeInsets.symmetric(
                    horizontal: 12, vertical: 10),
                child: Row(
                  children: [
                    Text(
                      'DEBUG LOG (${_debugLog.length})',
                      style: const TextStyle(
                        fontSize: 11,
                        fontWeight: FontWeight.w700,
                        color: Color(0xFF94A3B8),
                        letterSpacing: 0.8,
                      ),
                    ),
                    const Spacer(),
                    if (_debugExpanded)
                      GestureDetector(
                        onTap: () => setState(() {
                          _debugLog.clear();
                          _debugExpanded = false;
                        }),
                        child: const Padding(
                          padding: EdgeInsets.only(right: 12),
                          child: Text(
                            'CLEAR',
                            style: TextStyle(
                              fontSize: 11,
                              fontWeight: FontWeight.w700,
                              color: Color(0xFFEF4444),
                              letterSpacing: 0.8,
                            ),
                          ),
                        ),
                      ),
                    Text(
                      _debugExpanded ? '▼' : '▶',
                      style: const TextStyle(
                          fontSize: 11, color: Color(0xFF94A3B8)),
                    ),
                  ],
                ),
              ),
            ),
            if (_debugExpanded)
              Container(
                constraints: const BoxConstraints(maxHeight: 200),
                padding: const EdgeInsets.fromLTRB(12, 0, 12, 10),
                child: SingleChildScrollView(
                  reverse: true,
                  child: SelectableText(
                    _debugLog.join('\n'),
                    style: const TextStyle(
                      fontSize: 11,
                      fontFamily: 'monospace',
                      color: Color(0xFFCBD5E1),
                      height: 1.4,
                    ),
                  ),
                ),
              ),
          ],
        ),
      ),
    );
  }

  Widget _sectionLabel(String text) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 8),
      child: Text(
        text,
        style: const TextStyle(
          fontSize: 11,
          fontWeight: FontWeight.w600,
          color: Color(0xFF94A3B8),
          letterSpacing: 0.8,
        ),
      ),
    );
  }

  // ── State 1: Destination Picker ──────────────────────────────────────────

  Widget _buildDestinationPicker() {
    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        _buildHandle(),
        const Text(
          'Send File to Remote',
          style: TextStyle(
            fontSize: 18,
            fontWeight: FontWeight.w700,
            color: Color(0xFFE2E8F0),
          ),
        ),
        const SizedBox(height: 4),
        const Text(
          'Choose destination on remote machine',
          style: TextStyle(fontSize: 13, color: Color(0xFF94A3B8)),
        ),
        const SizedBox(height: 20),
        _sectionLabel('COMMON DESTINATIONS'),
        SizedBox(
          height: 40,
          child: ListView(
            scrollDirection: Axis.horizontal,
            children: _chips.map((chip) {
              final isSelected = _selectedChip == chip.$3;
              return Padding(
                padding: const EdgeInsets.only(right: 8),
                child: GestureDetector(
                  onTap: () {
                    setState(() {
                      _selectedChip = chip.$3;
                      _pathController.text = chip.$3;
                    });
                  },
                  child: Container(
                    padding: const EdgeInsets.symmetric(
                        horizontal: 12, vertical: 8),
                    decoration: BoxDecoration(
                      color: isSelected
                          ? const Color(0xFF2563EB).withValues(alpha: 0.15)
                          : const Color(0xFF1E293B),
                      borderRadius: BorderRadius.circular(20),
                      border: Border.all(
                        color: isSelected
                            ? const Color(0xFF2563EB)
                            : Colors.transparent,
                        width: 1.5,
                      ),
                    ),
                    child: Text(
                      '${chip.$1} ${chip.$2}',
                      style: TextStyle(
                        fontSize: 13,
                        color: isSelected
                            ? const Color(0xFF2563EB)
                            : const Color(0xFFE2E8F0),
                        fontWeight: isSelected
                            ? FontWeight.w600
                            : FontWeight.w400,
                      ),
                    ),
                  ),
                ),
              );
            }).toList(),
          ),
        ),
        const SizedBox(height: 16),
        _sectionLabel('CUSTOM PATH'),
        TextField(
          controller: _pathController,
          onChanged: (_) => setState(() => _selectedChip = null),
          style: const TextStyle(color: Color(0xFFE2E8F0), fontSize: 14),
          decoration: InputDecoration(
            filled: true,
            fillColor: const Color(0xFF1E293B),
            border: OutlineInputBorder(
              borderRadius: BorderRadius.circular(10),
              borderSide: BorderSide.none,
            ),
            contentPadding:
                const EdgeInsets.symmetric(horizontal: 14, vertical: 12),
            hintText: '/path/to/destination',
            hintStyle: const TextStyle(color: Color(0xFF64748B), fontSize: 14),
          ),
        ),
        const SizedBox(height: 12),
        // Destination summary bar
        Container(
          width: double.infinity,
          padding:
              const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
          decoration: BoxDecoration(
            color: const Color(0xFF2563EB).withValues(alpha: 0.1),
            borderRadius: BorderRadius.circular(8),
          ),
          child: Row(
            children: [
              const Text(
                'DESTINATION  ',
                style: TextStyle(
                  fontSize: 11,
                  fontWeight: FontWeight.w700,
                  color: Color(0xFF2563EB),
                  letterSpacing: 0.8,
                ),
              ),
              Expanded(
                child: Text(
                  _pathController.text.isEmpty ? '—' : _pathController.text,
                  style: const TextStyle(
                      fontSize: 13, color: Color(0xFFE2E8F0)),
                  overflow: TextOverflow.ellipsis,
                ),
              ),
            ],
          ),
        ),
        const SizedBox(height: 16),
        SizedBox(
          width: double.infinity,
          height: 48,
          child: ElevatedButton(
            onPressed: _pathController.text.trim().isEmpty
                ? null
                : _pickAndSend,
            style: ElevatedButton.styleFrom(
              backgroundColor: const Color(0xFF2563EB),
              disabledBackgroundColor: const Color(0xFF334155),
              shape: RoundedRectangleBorder(
                borderRadius: BorderRadius.circular(12),
              ),
            ),
            child: Text(
              'Choose File & Send',
              style: TextStyle(
                fontSize: 15,
                fontWeight: FontWeight.w600,
                color: _pathController.text.trim().isEmpty
                    ? const Color(0xFF64748B)
                    : Colors.white,
              ),
            ),
          ),
        ),
      ],
    );
  }

  // ── State 2: Sending ─────────────────────────────────────────────────────

  Widget _buildSending(List<JobProgress> jobs) {
    _checkAllDone(jobs);

    final matchedJobs = _jobIds
        .map((id) => jobs.firstWhereOrNull((j) => j.id == id))
        .whereType<JobProgress>()
        .toList();

    final totalFinished = matchedJobs.fold(0, (s, j) => s + j.finishedSize);
    final totalSize = matchedJobs.fold(0, (s, j) => s + j.totalSize);
    final totalPercent =
        totalSize > 0 ? totalFinished / totalSize : 0.0;
    final speed = matchedJobs.fold(0.0, (s, j) => s + j.speed);

    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        _buildHandle(),
        Text(
          'Sending ${_files.length == 1 ? 'File' : 'Files'}',
          style: const TextStyle(
            fontSize: 18,
            fontWeight: FontWeight.w700,
            color: Color(0xFFE2E8F0),
          ),
        ),
        const SizedBox(height: 4),
        Text(
          'To $_destination',
          style: const TextStyle(fontSize: 13, color: Color(0xFF94A3B8)),
          overflow: TextOverflow.ellipsis,
        ),
        const SizedBox(height: 20),
        if (_files.length == 1)
          _buildSingleFileSending(matchedJobs, totalPercent, speed)
        else
          _buildMultiFileSending(
              matchedJobs, totalFinished, totalSize, totalPercent, speed),
        const SizedBox(height: 16),
        SizedBox(
          width: double.infinity,
          height: 48,
          child: TextButton(
            onPressed: _cancel,
            style: TextButton.styleFrom(
              backgroundColor: const Color(0xFF1E293B),
              shape: RoundedRectangleBorder(
                borderRadius: BorderRadius.circular(12),
              ),
            ),
            child: const Text(
              'Cancel',
              style: TextStyle(
                fontSize: 15,
                fontWeight: FontWeight.w600,
                color: Color(0xFFEF4444),
              ),
            ),
          ),
        ),
      ],
    );
  }

  Widget _buildSingleFileSending(
      List<JobProgress> jobs, double totalPercent, double speed) {
    final file = _files.first;
    final job = jobs.isNotEmpty ? jobs.first : null;
    final finished = job?.finishedSize ?? 0;
    final total = job?.totalSize ?? file.size;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          children: [
            Text(
              _fileIcon(file.name),
              style: const TextStyle(fontSize: 28),
            ),
            const SizedBox(width: 12),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    file.name,
                    style: const TextStyle(
                        fontSize: 14,
                        fontWeight: FontWeight.w500,
                        color: Color(0xFFE2E8F0)),
                    overflow: TextOverflow.ellipsis,
                  ),
                  Text(
                    _formatBytes(file.size),
                    style: const TextStyle(
                        fontSize: 12, color: Color(0xFF94A3B8)),
                  ),
                ],
              ),
            ),
          ],
        ),
        const SizedBox(height: 12),
        ClipRRect(
          borderRadius: BorderRadius.circular(3),
          child: LinearProgressIndicator(
            value: totalPercent,
            minHeight: 6,
            backgroundColor: const Color(0xFF334155),
            valueColor:
                const AlwaysStoppedAnimation<Color>(Color(0xFF2563EB)),
          ),
        ),
        const SizedBox(height: 8),
        Row(
          mainAxisAlignment: MainAxisAlignment.spaceBetween,
          children: [
            Text(
              '${_formatBytes(finished)} / ${_formatBytes(total)}',
              style: const TextStyle(
                  fontSize: 12, color: Color(0xFF94A3B8)),
            ),
            Text(
              '${(totalPercent * 100).toStringAsFixed(0)}%',
              style: const TextStyle(
                fontSize: 13,
                fontWeight: FontWeight.w600,
                color: Color(0xFF2563EB),
              ),
            ),
          ],
        ),
        const SizedBox(height: 4),
        Text(
          '↑ ${_formatBytes(speed.toInt())}/s',
          style: const TextStyle(fontSize: 12, color: Color(0xFF94A3B8)),
        ),
      ],
    );
  }

  Widget _buildMultiFileSending(List<JobProgress> jobs, int totalFinished,
      int totalSize, double totalPercent, double speed) {
    final totalBytes = _files.fold(0, (s, f) => s + f.size);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        GestureDetector(
          onTap: () =>
              setState(() => _accordionExpanded = !_accordionExpanded),
          child: Container(
            padding: const EdgeInsets.all(14),
            decoration: BoxDecoration(
              color: const Color(0xFF1E293B),
              borderRadius: BorderRadius.circular(12),
            ),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  mainAxisAlignment: MainAxisAlignment.spaceBetween,
                  children: [
                    Text(
                      '${_files.length} files · ${_formatBytes(totalBytes)} total',
                      style: const TextStyle(
                        fontSize: 13,
                        fontWeight: FontWeight.w500,
                        color: Color(0xFFE2E8F0),
                      ),
                    ),
                    Text(
                      '${(totalPercent * 100).toStringAsFixed(0)}% ${_accordionExpanded ? '▼' : '▶'}',
                      style: const TextStyle(
                        fontSize: 13,
                        fontWeight: FontWeight.w600,
                        color: Color(0xFF2563EB),
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 10),
                ClipRRect(
                  borderRadius: BorderRadius.circular(3),
                  child: LinearProgressIndicator(
                    value: totalPercent,
                    minHeight: 6,
                    backgroundColor: const Color(0xFF334155),
                    valueColor: const AlwaysStoppedAnimation<Color>(
                        Color(0xFF2563EB)),
                  ),
                ),
                const SizedBox(height: 8),
                Row(
                  mainAxisAlignment: MainAxisAlignment.spaceBetween,
                  children: [
                    Text(
                      '${_formatBytes(totalFinished)} / ${_formatBytes(totalSize)}',
                      style: const TextStyle(
                          fontSize: 12, color: Color(0xFF94A3B8)),
                    ),
                    Text(
                      '↑ ${_formatBytes(speed.toInt())}/s',
                      style: const TextStyle(
                          fontSize: 12, color: Color(0xFF94A3B8)),
                    ),
                  ],
                ),
                if (!_accordionExpanded) ...[
                  const SizedBox(height: 8),
                  const Text(
                    'Tap to see individual files',
                    style: TextStyle(
                        fontSize: 11, color: Color(0xFF64748B)),
                  ),
                ],
                if (_accordionExpanded) ...[
                  const SizedBox(height: 12),
                  const Divider(color: Color(0xFF3A3A3C), height: 1),
                  const SizedBox(height: 10),
                  SizedBox(
                    height: 240,
                    child: ListView.builder(
                      itemCount: _files.length,
                      padding: EdgeInsets.zero,
                      itemBuilder: (_, i) {
                        final jobId = i < _jobIds.length ? _jobIds[i] : -1;
                        final job = jobs.firstWhereOrNull((j) => j.id == jobId);
                        return _buildFileRow(_files[i], job);
                      },
                    ),
                  ),
                ],
              ],
            ),
          ),
        ),
      ],
    );
  }

  Widget _buildFileRow(PlatformFile file, JobProgress? job) {
    Color badgeColor;
    String badgeText;
    if (job == null || job.state == JobState.none) {
      badgeColor = const Color(0xFF64748B);
      badgeText = 'Queued';
    } else if (job.state == JobState.done) {
      badgeColor = const Color(0xFF22C55E);
      badgeText = '✓ Done';
    } else if (job.state == JobState.error) {
      badgeColor = const Color(0xFFEF4444);
      badgeText = '✗ Error';
    } else {
      badgeColor = const Color(0xFF2563EB);
      badgeText = job.percentText;
    }

    final filePercent = job != null && job.totalSize > 0
        ? job.finishedSize / job.totalSize
        : 0.0;

    return Padding(
      padding: const EdgeInsets.only(bottom: 10),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Text(_fileIcon(file.name),
                  style: const TextStyle(fontSize: 18)),
              const SizedBox(width: 8),
              Expanded(
                child: Text(
                  file.name,
                  style: const TextStyle(
                      fontSize: 13, color: Color(0xFFE2E8F0)),
                  overflow: TextOverflow.ellipsis,
                ),
              ),
              const SizedBox(width: 8),
              Container(
                padding:
                    const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
                decoration: BoxDecoration(
                  color: badgeColor.withValues(alpha: 0.15),
                  borderRadius: BorderRadius.circular(6),
                ),
                child: Text(
                  badgeText,
                  style: TextStyle(
                    fontSize: 11,
                    fontWeight: FontWeight.w600,
                    color: badgeColor,
                  ),
                ),
              ),
            ],
          ),
          const SizedBox(height: 4),
          ClipRRect(
            borderRadius: BorderRadius.circular(2),
            child: LinearProgressIndicator(
              value: filePercent,
              minHeight: 4,
              backgroundColor: const Color(0xFF334155),
              valueColor: AlwaysStoppedAnimation<Color>(badgeColor),
            ),
          ),
        ],
      ),
    );
  }

  // ── State 3: Done ────────────────────────────────────────────────────────

  Widget _buildDone(List<JobProgress> jobs) {
    final doneCount =
        _jobIds.where((id) {
          final job = jobs.firstWhereOrNull((j) => j.id == id);
          return job?.state == JobState.done;
        }).length;
    final errorCount =
        _jobIds.where((id) {
          final job = jobs.firstWhereOrNull((j) => j.id == id);
          return job?.state == JobState.error;
        }).length;
    final totalBytes = _jobIds.fold(0, (s, id) {
      final job = jobs.firstWhereOrNull((j) => j.id == id);
      return s + (job?.finishedSize ?? 0);
    });

    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        _buildHandle(),
        Text(
          '${_files.length == 1 ? 'File' : 'Files'} Sent',
          style: const TextStyle(
            fontSize: 18,
            fontWeight: FontWeight.w700,
            color: Colors.white,
          ),
        ),
        const SizedBox(height: 4),
        Text(
          'To $_destination',
          style: const TextStyle(fontSize: 13, color: Color(0xFF94A3B8)),
          overflow: TextOverflow.ellipsis,
        ),
        const SizedBox(height: 24),
        Center(
          child: Container(
            width: 64,
            height: 64,
            decoration: BoxDecoration(
              color: const Color(0xFF22C55E).withValues(alpha: 0.22),
              borderRadius: BorderRadius.circular(16),
            ),
            child: const Center(
              child: Text('✓',
                  style: TextStyle(
                      fontSize: 32,
                      color: Color(0xFF22C55E),
                      fontWeight: FontWeight.w700)),
            ),
          ),
        ),
        const SizedBox(height: 16),
        Center(
          child: Text(
            _files.length == 1
                ? _files.first.name
                : '$doneCount of ${_files.length} files sent successfully',
            style: const TextStyle(
              fontSize: 14,
              fontWeight: FontWeight.w500,
              color: Color(0xFFE2E8F0),
            ),
            textAlign: TextAlign.center,
          ),
        ),
        if (totalBytes > 0) ...[
          const SizedBox(height: 4),
          Center(
            child: Text(
              _formatBytes(totalBytes),
              style: const TextStyle(
                  fontSize: 12, color: Color(0xFF94A3B8)),
            ),
          ),
        ],
        const SizedBox(height: 20),
        SizedBox(
          width: double.infinity,
          height: 44,
          child: TextButton(
            onPressed: () {
              setState(() {
                _sheetState = _SheetState.destinationPicker;
                _files = [];
                _jobIds = [];
                _accordionExpanded = false;
              });
            },
            style: TextButton.styleFrom(
              backgroundColor: const Color(0xFF1E293B),
              shape: RoundedRectangleBorder(
                borderRadius: BorderRadius.circular(12),
              ),
            ),
            child: const Text(
              'Send More Files',
              style: TextStyle(
                  fontSize: 15,
                  fontWeight: FontWeight.w600,
                  color: Color(0xFFE2E8F0)),
            ),
          ),
        ),
        if (errorCount > 0) ...[
          const SizedBox(height: 8),
          SizedBox(
            width: double.infinity,
            height: 44,
            child: TextButton(
              onPressed: () => _retryFailed(jobs),
              style: TextButton.styleFrom(
                backgroundColor: const Color(0xFF1E293B),
                shape: RoundedRectangleBorder(
                  borderRadius: BorderRadius.circular(12),
                ),
              ),
              child: const Text(
                'Retry Failed',
                style: TextStyle(
                    fontSize: 15,
                    fontWeight: FontWeight.w600,
                    color: Color(0xFFEF4444)),
              ),
            ),
          ),
        ],
        const SizedBox(height: 16),
        SizedBox(
          width: double.infinity,
          height: 48,
          child: ElevatedButton(
            onPressed: () => Navigator.of(context).pop(),
            style: ElevatedButton.styleFrom(
              backgroundColor: const Color(0xFF22C55E),
              shape: RoundedRectangleBorder(
                borderRadius: BorderRadius.circular(12),
              ),
            ),
            child: const Text(
              'Done',
              style: TextStyle(
                  fontSize: 15,
                  fontWeight: FontWeight.w700,
                  color: Colors.white),
            ),
          ),
        ),
      ],
    );
  }

  // ── Root build ───────────────────────────────────────────────────────────

  @override
  Widget build(BuildContext context) {
    return SafeArea(
      child: Padding(
        padding: EdgeInsets.fromLTRB(
          16,
          16,
          16,
          MediaQuery.of(context).viewInsets.bottom + 16,
        ),
        child: SingleChildScrollView(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              _sheetState == _SheetState.destinationPicker
                  ? _buildDestinationPicker()
                  : Obx(() {
                      final jobs =
                          widget.ffi.fileModel.jobController.jobTable;
                      return _sheetState == _SheetState.sending
                          ? _buildSending(jobs)
                          : _buildDone(jobs);
                    }),
              _buildDebugLog(),
            ],
          ),
        ),
      ),
    );
  }
}
