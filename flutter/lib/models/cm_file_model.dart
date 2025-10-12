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
  final currentJobTable = RxList<CmFileLog>();
  final _jobTables = HashMap<int, RxList<CmFileLog>>.fromEntries([]);
  Stopwatch stopwatch = Stopwatch();
  int _lastElapsed = 0;

  CmFileModel(this.parent);

  void updateCurrentClientId(int id) {
    if (_jobTables[id] == null) {
      _jobTables[id] = RxList<CmFileLog>();
    }
    Future.delayed(Duration.zero, () {
      currentJobTable.value = _jobTables[id]!;
    });
  }

  onFileTransferLog(Map<String, dynamic> evt) {
    if (evt['transfer'] != null) {
      _onFileTransfer(evt['transfer']);
    } else if (evt['remove'] != null) {
      _onFileRemove(evt['remove']);
    } else if (evt['create_dir'] != null) {
      _onDirCreate(evt['create_dir']);
    } else if (evt['rename'] != null) {
      _onRename(evt['rename']);
    }
  }

  _onFileTransfer(dynamic log) {
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
    var jobTable = _jobTables[data.connId];
    if (jobTable == null) {
      debugPrint("jobTable should not be null");
      return;
    }
    CmFileLog? job = jobTable.firstWhereOrNull((e) => e.id == data.id);
    if (job == null) {
      job = CmFileLog();
      jobTable.add(job);
      _addUnread(data.connId);
    }
    job.id = data.id;
    job.action =
        data.isRemote ? CmFileAction.remoteToLocal : CmFileAction.localToRemote;
    job.fileName = data.path;
    job.totalSize = data.totalSize;
    job.finishedSize = data.finishedSize;
    if (job.finishedSize > data.totalSize) {
      job.finishedSize = data.totalSize;
    }

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

  _onFileRemove(dynamic log) {
    try {
      dynamic d = jsonDecode(log);
      FileActionLog data = FileActionLog.fromJson(d);
      Client? client =
          gFFI.serverModel.clients.firstWhereOrNull((e) => e.id == data.connId);
      var jobTable = _jobTables[data.connId];
      if (jobTable == null) {
        debugPrint("jobTable should not be null");
        return;
      }
      int removeUnreadCount = 0;
      if (data.dir) {
        bool isChild(String parent, String child) {
          if (child.startsWith(parent) && child.length > parent.length) {
            final suffix = child.substring(parent.length);
            return suffix.startsWith('/') || suffix.startsWith('\\');
          }
          return false;
        }

        removeUnreadCount = jobTable
            .where((e) =>
                e.action == CmFileAction.remove &&
                isChild(data.path, e.fileName))
            .length;
        jobTable.removeWhere((e) =>
            e.action == CmFileAction.remove && isChild(data.path, e.fileName));
      }
      jobTable.add(CmFileLog()
        ..id = data.id
        ..fileName = data.path
        ..action = CmFileAction.remove
        ..state = JobState.done);
      final currentSelectedTab =
          gFFI.serverModel.tabController.state.value.selectedTabInfo;
      if (!(gFFI.chatModel.isShowCMSidePage &&
          currentSelectedTab.key == data.connId.toString())) {
        // Wrong number if unreadCount changes during deletion, which rarely happens
        RxInt? rx = client?.unreadChatMessageCount;
        if (rx != null) {
          if (rx.value >= removeUnreadCount) {
            rx.value -= removeUnreadCount;
          }
          rx.value += 1;
        }
      }
      jobTable.refresh();
    } catch (e) {
      debugPrint('$e');
    }
  }

  _onDirCreate(dynamic log) {
    try {
      dynamic d = jsonDecode(log);
      FileActionLog data = FileActionLog.fromJson(d);
      var jobTable = _jobTables[data.connId];
      if (jobTable == null) {
        debugPrint("jobTable should not be null");
        return;
      }
      jobTable.add(CmFileLog()
        ..id = data.id
        ..fileName = data.path
        ..action = CmFileAction.createDir
        ..state = JobState.done);
      _addUnread(data.connId);
      jobTable.refresh();
    } catch (e) {
      debugPrint('$e');
    }
  }

  _onRename(dynamic log) {
    try {
      dynamic d = jsonDecode(log);
      FileRenamenLog data = FileRenamenLog.fromJson(d);
      var jobTable = _jobTables[data.connId];
      if (jobTable == null) {
        debugPrint("jobTable should not be null");
        return;
      }
      final fileName = '${data.path} -> ${data.newName}';
      jobTable.add(CmFileLog()
        ..id = 0
        ..fileName = fileName
        ..action = CmFileAction.rename
        ..state = JobState.done);
      _addUnread(data.connId);
      jobTable.refresh();
    } catch (e) {
      debugPrint('$e');
    }
  }

  _addUnread(int connId) {
    Client? client =
        gFFI.serverModel.clients.firstWhereOrNull((e) => e.id == connId);
    final currentSelectedTab =
        gFFI.serverModel.tabController.state.value.selectedTabInfo;
    if (!(gFFI.chatModel.isShowCMSidePage &&
        currentSelectedTab.key == connId.toString())) {
      client?.unreadChatMessageCount.value += 1;
    }
  }
}

enum CmFileAction {
  none,
  remoteToLocal,
  localToRemote,
  remove,
  createDir,
  rename,
}

class CmFileLog {
  JobState state = JobState.none;
  var id = 0;
  var speed = 0.0;
  var finishedSize = 0;
  var totalSize = 0;
  CmFileAction action = CmFileAction.none;
  var fileName = "";
  var err = "";
  int lastTransferredSize = 0;

  String display() {
    if (state == JobState.done && err == "skipped") {
      return translate("Skipped");
    }
    return state.display();
  }

  bool isTransfer() {
    return action == CmFileAction.remoteToLocal ||
        action == CmFileAction.localToRemote;
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

class FileActionLog {
  int id = 0;
  int connId = 0;
  String path = '';
  bool dir = false;

  FileActionLog({
    required this.connId,
    required this.id,
    required this.path,
    required this.dir,
  });

  FileActionLog.fromJson(dynamic d)
      : this(
          connId: d['connId'] ?? 0,
          id: d['id'] ?? 0,
          path: d['path'] ?? '',
          dir: d['dir'] ?? false,
        );
}

class FileRenamenLog {
  int connId = 0;
  String path = '';
  String newName = '';

  FileRenamenLog({
    required this.connId,
    required this.path,
    required this.newName,
  });

  FileRenamenLog.fromJson(dynamic d)
      : this(
          connId: d['connId'] ?? 0,
          path: d['path'] ?? '',
          newName: d['newName'] ?? '',
        );
}
