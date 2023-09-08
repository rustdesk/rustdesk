import 'dart:collection';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/models/server_model.dart';
import 'package:get/get.dart';
import 'file_model.dart';

class CmFileModel {
  final WeakReference<FFI> parent;
  final currentJobTable = RxList<JobProgress>();
  final _jobTables = HashMap<int, RxList<JobProgress>>.fromEntries([]);
  Stopwatch stopwatch = Stopwatch();
  int _lastElapsed = 0;

  CmFileModel(this.parent);

  void updateCurrentClientId(int id) {
    if (_jobTables[id] == null) {
      _jobTables[id] = RxList<JobProgress>();
    }
    Future.delayed(Duration.zero, () {
      currentJobTable.value = _jobTables[id]!;
    });
  }

  onFileTransferLog(dynamic log) {
    try {
      dynamic d = jsonDecode(log);
      if (!stopwatch.isRunning) stopwatch.start();
      bool calcSpeed = stopwatch.elapsedMilliseconds - _lastElapsed >= 1000;
      if (calcSpeed) {
        _lastElapsed = stopwatch.elapsedMilliseconds;
      }
      if (d is List<dynamic>) {
        for (var l in d) {
          _dealOneJob(l, calcSpeed);
        }
      } else {
        _dealOneJob(d, calcSpeed);
      }
      currentJobTable.refresh();
    } catch (e) {
      debugPrint("onFileTransferLog:$e");
    }
  }

  _dealOneJob(dynamic l, bool calcSpeed) {
    final data = TransferJobSerdeData.fromJson(l);
    Client? client =
        gFFI.serverModel.clients.firstWhereOrNull((e) => e.id == data.connId);
    var jobTable = _jobTables[data.connId];
    if (jobTable == null) {
      debugPrint("jobTable should not be null");
      return;
    }
    JobProgress? job = jobTable.firstWhereOrNull((e) => e.id == data.id);
    if (job == null) {
      job = JobProgress();
      jobTable.add(job);
      final currentSelectedTab =
          gFFI.serverModel.tabController.state.value.selectedTabInfo;
      if (!(gFFI.chatModel.isShowCMSidePage &&
          currentSelectedTab.key == data.connId.toString())) {
        client?.unreadChatMessageCount.value += 1;
      }
    }
    job.id = data.id;
    job.isRemoteToLocal = data.isRemote;
    job.fileName = data.path;
    job.totalSize = data.totalSize;
    job.finishedSize = data.finishedSize;
    if (job.finishedSize > data.totalSize) {
      job.finishedSize = data.totalSize;
    }
    job.isRemoteToLocal = data.isRemote;

    if (job.finishedSize > 0) {
      if (job.finishedSize < job.totalSize) {
        job.state = JobState.inProgress;
      } else {
        job.state = JobState.done;
      }
    }
    if (data.done) {
      job.state = JobState.done;
    } else if (data.cancel || data.error == 'skipped') {
      job.state = JobState.done;
      job.err = 'skipped';
    } else if (data.error.isNotEmpty) {
      job.state = JobState.error;
      job.err = data.error;
    }
    if (calcSpeed) {
      job.speed = (data.transferred - job.lastTransferredSize) * 1.0;
      job.lastTransferredSize = data.transferred;
    }
    jobTable.refresh();
  }
}

class TransferJobSerdeData {
  int connId;
  int id;
  String path;
  bool isRemote;
  int totalSize;
  int finishedSize;
  int transferred;
  bool done;
  bool cancel;
  String error;

  TransferJobSerdeData({
    required this.connId,
    required this.id,
    required this.path,
    required this.isRemote,
    required this.totalSize,
    required this.finishedSize,
    required this.transferred,
    required this.done,
    required this.cancel,
    required this.error,
  });

  TransferJobSerdeData.fromJson(dynamic d)
      : this(
          connId: d['connId'] ?? 0,
          id: int.tryParse(d['id'].toString()) ?? 0,
          path: d['path'] ?? '',
          isRemote: d['isRemote'] ?? false,
          totalSize: d['totalSize'] ?? 0,
          finishedSize: d['finishedSize'] ?? 0,
          transferred: d['transferred'] ?? 0,
          done: d['done'] ?? false,
          cancel: d['cancel'] ?? false,
          error: d['error'] ?? '',
        );
}
