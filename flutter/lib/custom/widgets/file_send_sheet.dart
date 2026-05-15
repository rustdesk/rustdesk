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

  static const _chips = [
    ('🏠', 'Home', '~'),
    ('🖥', 'Desktop', '~/Desktop'),
    ('⬇️', 'Downloads', '~/Downloads'),
    ('📄', 'Documents', '~/Documents'),
    ('🗂', '/tmp', '/tmp'),
  ];

  @override
  void dispose() {
    _pathController.dispose();
    super.dispose();
  }

  // ── Transfer initiation ──────────────────────────────────────────────────

  Future<void> _startTransfer(List<PlatformFile> files) async {
    final remoteCtrl = widget.ffi.fileModel.remoteController;
    final remoteHome = remoteCtrl.homePath;
    final isRemoteWindows = remoteCtrl.options.value.isWindows;

    if (remoteHome.isEmpty) {
      // Remote home not yet known — first remote directory listing populates it
      // via receiveFileDir/initDirAndHome.
      ScaffoldMessenger.of(context).showSnackBar(const SnackBar(
        content: Text('Connecting to remote file system… try again in a moment.'),
        duration: Duration(seconds: 2),
      ));
      return;
    }

    final rawDest = _pathController.text.trim();
    final destDir = rawDest.startsWith('~')
        ? rawDest.replaceFirst('~', remoteHome)
        : rawDest;

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

      await bind.sessionSendFiles(
        sessionId: widget.ffi.sessionId,
        actId: jobId,
        path: file.path!,
        to: PathUtil.join(destDir, entry.name, isRemoteWindows),
        fileNum: 0,
        includeHidden: false,
        isRemote: false,
        isDir: false,
      );
    }

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
                : () async {
                    final result =
                        await FilePicker.platform.pickFiles(
                      allowMultiple: true,
                    );
                    if (result != null && result.files.isNotEmpty) {
                      await _startTransfer(result.files);
                    }
                  },
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
        child: _sheetState == _SheetState.destinationPicker
            ? _buildDestinationPicker()
            : Obx(() {
                final jobs = widget.ffi.fileModel.jobController.jobTable;
                return _sheetState == _SheetState.sending
                    ? _buildSending(jobs)
                    : _buildDone(jobs);
              }),
      ),
    );
  }
}
