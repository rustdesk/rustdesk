import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/utils/event_loop.dart';
import 'package:get/get.dart';
import 'package:path/path.dart' as path;

import '../consts.dart';
import 'model.dart';
import 'platform_model.dart';

enum SortBy {
  name,
  type,
  modified,
  size;

  @override
  String toString() {
    final str = this.name.toString();
    return "${str[0].toUpperCase()}${str.substring(1)}";
  }
}

class JobID {
  int _count = 0;
  int next() {
    _count++;
    return _count;
  }
}

typedef GetSessionID = SessionID Function();

class FileModel {
  final WeakReference<FFI> parent;
  // late final String sessionId;
  late final FileFetcher fileFetcher;
  late final JobController jobController;

  late final FileController localController;
  late final FileController remoteController;

  late final GetSessionID getSessionID;
  SessionID get sessionId => getSessionID();
  late final FileDialogEventLoop evtLoop;

  FileModel(this.parent) {
    getSessionID = () => parent.target!.sessionId;
    fileFetcher = FileFetcher(getSessionID);
    jobController = JobController(getSessionID);
    localController = FileController(
        isLocal: true,
        getSessionID: getSessionID,
        rootState: parent,
        jobController: jobController,
        fileFetcher: fileFetcher,
        getOtherSideDirectoryData: () => remoteController.directoryData());
    remoteController = FileController(
        isLocal: false,
        getSessionID: getSessionID,
        rootState: parent,
        jobController: jobController,
        fileFetcher: fileFetcher,
        getOtherSideDirectoryData: () => localController.directoryData());
    evtLoop = FileDialogEventLoop();
  }

  Future<void> onReady() async {
    await evtLoop.onReady();
    await localController.onReady();
    await remoteController.onReady();
  }

  Future<void> close() async {
    await evtLoop.close();
    parent.target?.dialogManager.dismissAll();
    await localController.close();
    await remoteController.close();
  }

  Future<void> refreshAll() async {
    await localController.refresh();
    await remoteController.refresh();
  }

  void receiveFileDir(Map<String, dynamic> evt) {
    if (evt['is_local'] == "false") {
      // init remote home, the remote connection will send one dir event when established. TODO opt
      remoteController.initDirAndHome(evt);
    }
    fileFetcher.tryCompleteTask(evt['value'], evt['is_local']);
  }

  Future<void> postOverrideFileConfirm(Map<String, dynamic> evt) async {
    evtLoop.pushEvent(
        _FileDialogEvent(WeakReference(this), FileDialogType.overwrite, evt));
  }

  Future<void> overrideFileConfirm(Map<String, dynamic> evt,
      {bool? overrideConfirm, bool skip = false}) async {
    // If `skip == true`, it means to skip this file without showing dialog.
    // Because `resp` may be null after the user operation or the last remembered operation,
    // and we should distinguish them.
    final resp = overrideConfirm ??
        (!skip
            ? await showFileConfirmDialog(translate("Overwrite"),
                "${evt['read_path']}", true, evt['is_identical'] == "true")
            : null);
    final id = int.tryParse(evt['id']) ?? 0;
    if (false == resp) {
      final jobIndex = jobController.getJob(id);
      if (jobIndex != -1) {
        await jobController.cancelJob(id);
        final job = jobController.jobTable[jobIndex];
        job.state = JobState.done;
        jobController.jobTable.refresh();
      }
    } else {
      var need_override = false;
      if (resp == null) {
        // skip
        need_override = false;
      } else {
        // overwrite
        need_override = true;
      }
      // Update the loop config.
      if (fileConfirmCheckboxRemember) {
        evtLoop.setSkip(!need_override);
      }
      await bind.sessionSetConfirmOverrideFile(
          sessionId: sessionId,
          actId: id,
          fileNum: int.parse(evt['file_num']),
          needOverride: need_override,
          remember: fileConfirmCheckboxRemember,
          isUpload: evt['is_upload'] == "true");
    }
    // Update the loop config.
    if (fileConfirmCheckboxRemember) {
      evtLoop.setOverrideConfirm(resp);
    }
  }

  bool fileConfirmCheckboxRemember = false;

  Future<bool?> showFileConfirmDialog(
      String title, String content, bool showCheckbox, bool isIdentical) async {
    fileConfirmCheckboxRemember = false;
    return await parent.target?.dialogManager.show<bool?>(
        (setState, Function(bool? v) close, context) {
      cancel() => close(false);
      submit() => close(true);
      return CustomAlertDialog(
        title: Row(
          children: [
            const Icon(Icons.warning_rounded, color: Colors.red),
            Text(title).paddingOnly(
              left: 10,
            ),
          ],
        ),
        contentBoxConstraints:
            BoxConstraints(minHeight: 100, minWidth: 400, maxWidth: 400),
        content: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            mainAxisSize: MainAxisSize.min,
            children: [
              Text(translate("This file exists, skip or overwrite this file?"),
                  style: const TextStyle(fontWeight: FontWeight.bold)),
              const SizedBox(height: 5),
              Text(content),
              Offstage(
                offstage: !isIdentical,
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    const SizedBox(height: 12),
                    Text(translate("identical_file_tip"),
                        style: const TextStyle(fontWeight: FontWeight.w500))
                  ],
                ),
              ),
              showCheckbox
                  ? CheckboxListTile(
                      contentPadding: const EdgeInsets.all(0),
                      dense: true,
                      controlAffinity: ListTileControlAffinity.leading,
                      title: Text(
                        translate("Do this for all conflicts"),
                      ),
                      value: fileConfirmCheckboxRemember,
                      onChanged: (v) {
                        if (v == null) return;
                        setState(() => fileConfirmCheckboxRemember = v);
                      },
                    )
                  : const SizedBox.shrink()
            ]),
        actions: [
          dialogButton(
            "Cancel",
            icon: Icon(Icons.close_rounded),
            onPressed: cancel,
            isOutline: true,
          ),
          dialogButton(
            "Skip",
            icon: Icon(Icons.navigate_next_rounded),
            onPressed: () => close(null),
            isOutline: true,
          ),
          dialogButton(
            "OK",
            icon: Icon(Icons.done_rounded),
            onPressed: submit,
          ),
        ],
        onSubmit: submit,
        onCancel: cancel,
      );
    }, useAnimation: false);
  }
}

class DirectoryData {
  final DirectoryOptions options;
  final FileDirectory directory;
  DirectoryData(this.directory, this.options);
}

class FileController {
  final bool isLocal;
  final GetSessionID getSessionID;
  SessionID get sessionId => getSessionID();

  final FileFetcher fileFetcher;

  final options = DirectoryOptions().obs;
  final directory = FileDirectory().obs;

  final history = RxList<String>.empty(growable: true);
  final sortBy = SortBy.name.obs;
  var sortAscending = true;
  final JobController jobController;
  final WeakReference<FFI> rootState;

  final DirectoryData Function() getOtherSideDirectoryData;
  late final SelectedItems selectedItems = SelectedItems(isLocal: isLocal);

  FileController(
      {required this.isLocal,
      required this.getSessionID,
      required this.rootState,
      required this.jobController,
      required this.fileFetcher,
      required this.getOtherSideDirectoryData});

  String get homePath => options.value.home;
  void set homePath(String path) => options.value.home = path;
  OverlayDialogManager? get dialogManager => rootState.target?.dialogManager;

  String get shortPath {
    final dirPath = directory.value.path;
    if (dirPath.startsWith(homePath)) {
      var path = dirPath.replaceFirst(homePath, "");
      if (path.isEmpty) return "";
      if (path[0] == "/" || path[0] == "\\") {
        // remove more '/' or '\'
        path = path.replaceFirst(path[0], "");
      }
      return path;
    } else {
      return dirPath.replaceFirst(homePath, "");
    }
  }

  DirectoryData directoryData() {
    return DirectoryData(directory.value, options.value);
  }

  Future<void> onReady() async {
    if (isLocal) {
      options.value.home = await bind.mainGetHomeDir();
    }
    options.value.showHidden = (await bind.sessionGetPeerOption(
            sessionId: sessionId,
            name: isLocal ? "local_show_hidden" : "remote_show_hidden"))
        .isNotEmpty;
    options.value.isWindows = isLocal
        ? isWindows
        : rootState.target?.ffiModel.pi.platform == kPeerPlatformWindows;

    await Future.delayed(Duration(milliseconds: 100));

    final dir = (await bind.sessionGetPeerOption(
        sessionId: sessionId, name: isLocal ? "local_dir" : "remote_dir"));
    openDirectory(dir.isEmpty ? options.value.home : dir);

    await Future.delayed(Duration(seconds: 1));

    if (directory.value.path.isEmpty) {
      openDirectory(options.value.home);
    }
  }

  Future<void> close() async {
    // save config
    Map<String, String> msgMap = {};
    msgMap[isLocal ? "local_dir" : "remote_dir"] = directory.value.path;
    msgMap[isLocal ? "local_show_hidden" : "remote_show_hidden"] =
        options.value.showHidden ? "Y" : "";
    for (final msg in msgMap.entries) {
      await bind.sessionPeerOption(
          sessionId: sessionId, name: msg.key, value: msg.value);
    }
    directory.value.clear();
    options.value.clear();
  }

  void toggleShowHidden({bool? showHidden}) {
    options.value.showHidden = showHidden ?? !options.value.showHidden;
    refresh();
  }

  void changeSortStyle(SortBy sort, {bool? isLocal, bool ascending = true}) {
    sortBy.value = sort;
    sortAscending = ascending;
    directory.update((dir) {
      dir?.changeSortStyle(sort, ascending: ascending);
    });
  }

  Future<void> refresh() async {
    await openDirectory(directory.value.path);
  }

  Future<void> openDirectory(String path, {bool isBack = false}) async {
    if (path == ".") {
      refresh();
      return;
    }
    if (path == "..") {
      goToParentDirectory();
      return;
    }
    if (!isBack) {
      pushHistory();
    }
    final showHidden = options.value.showHidden;
    final isWindows = options.value.isWindows;
    // process /C:\ -> C:\ on Windows
    if (isWindows && path.length > 1 && path[0] == '/') {
      path = path.substring(1);
      if (path[path.length - 1] != '\\') {
        path = "$path\\";
      }
    }
    try {
      final fd = await fileFetcher.fetchDirectory(path, isLocal, showHidden);
      fd.format(isWindows, sort: sortBy.value);
      directory.value = fd;
    } catch (e) {
      debugPrint("Failed to openDirectory $path: $e");
    }
  }

  void pushHistory() {
    if (history.isNotEmpty && history.last == directory.value.path) {
      return;
    }
    history.add(directory.value.path);
  }

  void goToHomeDirectory() {
    if (isLocal) {
      openDirectory(homePath);
      return;
    }
    homePath = "";
    openDirectory(homePath);
  }

  void goBack() {
    if (history.isEmpty) return;
    final path = history.removeAt(history.length - 1);
    if (path.isEmpty) return;
    if (directory.value.path == path) {
      goBack();
      return;
    }
    openDirectory(path, isBack: true);
  }

  void goToParentDirectory() {
    final isWindows = options.value.isWindows;
    final dirPath = directory.value.path;
    var parent = PathUtil.dirname(dirPath, isWindows);
    // specially for C:\, D:\, goto '/'
    if (parent == dirPath && isWindows) {
      openDirectory('/');
      return;
    }
    openDirectory(parent);
  }

  // TODO deprecated this
  void initDirAndHome(Map<String, dynamic> evt) {
    try {
      final fd = FileDirectory.fromJson(jsonDecode(evt['value']));
      fd.format(options.value.isWindows, sort: sortBy.value);
      if (fd.id > 0) {
        final jobIndex = jobController.getJob(fd.id);
        if (jobIndex != -1) {
          final job = jobController.jobTable[jobIndex];
          var totalSize = 0;
          var fileCount = fd.entries.length;
          for (var element in fd.entries) {
            totalSize += element.size;
          }
          job.totalSize = totalSize;
          job.fileCount = fileCount;
          debugPrint("update receive details: ${fd.path}");
          jobController.jobTable.refresh();
        }
      } else if (options.value.home.isEmpty) {
        options.value.home = fd.path;
        debugPrint("init remote home: ${fd.path}");
        directory.value = fd;
      }
    } catch (e) {
      debugPrint("initDirAndHome err=$e");
    }
  }

  /// sendFiles from current side (FileController.isLocal) to other side (SelectedItems).
  void sendFiles(SelectedItems items, DirectoryData otherSideData) {
    /// ignore wrong items side status
    if (items.isLocal != isLocal) {
      return;
    }

    // alias
    final isRemoteToLocal = !isLocal;

    final toPath = otherSideData.directory.path;
    final isWindows = otherSideData.options.isWindows;
    final showHidden = otherSideData.options.showHidden;
    for (var from in items.items) {
      final jobID = jobController.add(from, isRemoteToLocal);
      bind.sessionSendFiles(
          sessionId: sessionId,
          actId: jobID,
          path: from.path,
          to: PathUtil.join(toPath, from.name, isWindows),
          fileNum: 0,
          includeHidden: showHidden,
          isRemote: isRemoteToLocal);
      debugPrint(
          "path: ${from.path}, toPath: $toPath, to: ${PathUtil.join(toPath, from.name, isWindows)}");
    }
  }

  bool _removeCheckboxRemember = false;

  Future<void> removeAction(SelectedItems items) async {
    _removeCheckboxRemember = false;
    if (items.isLocal != isLocal) {
      debugPrint("Failed to removeFile, wrong files");
      return;
    }
    final isWindows = options.value.isWindows;
    await Future.forEach(items.items, (Entry item) async {
      final jobID = JobController.jobID.next();
      var title = "";
      var content = "";
      late final List<Entry> entries;
      if (item.isFile) {
        title = translate("Are you sure you want to delete this file?");
        content = item.name;
        entries = [item];
      } else if (item.isDirectory) {
        title = translate("Not an empty directory");
        dialogManager?.showLoading(translate("Waiting"));
        final fd = await fileFetcher.fetchDirectoryRecursive(
            jobID, item.path, items.isLocal, true);
        if (fd.path.isEmpty) {
          fd.path = item.path;
        }
        fd.format(isWindows);
        dialogManager?.dismissAll();
        if (fd.entries.isEmpty) {
          final confirm = await showRemoveDialog(
              translate(
                  "Are you sure you want to delete this empty directory?"),
              item.name,
              false);
          if (confirm == true) {
            sendRemoveEmptyDir(item.path, 0);
          }
          return;
        }
        entries = fd.entries;
      } else {
        entries = [];
      }

      for (var i = 0; i < entries.length; i++) {
        final dirShow = item.isDirectory
            ? "${translate("Are you sure you want to delete the file of this directory?")}\n"
            : "";
        final count = entries.length > 1 ? "${i + 1}/${entries.length}" : "";
        content = "$dirShow\n\n${entries[i].path}".trim();
        final confirm = await showRemoveDialog(
          count.isEmpty ? title : "$title ($count)",
          content,
          item.isDirectory,
        );
        try {
          if (confirm == true) {
            sendRemoveFile(entries[i].path, i);
            final res = await jobController.jobResultListener.start();
            // handle remove res;
            if (item.isDirectory &&
                res['file_num'] == (entries.length - 1).toString()) {
              sendRemoveEmptyDir(item.path, i);
            }
          }
          if (_removeCheckboxRemember) {
            if (confirm == true) {
              for (var j = i + 1; j < entries.length; j++) {
                sendRemoveFile(entries[j].path, j);
                final res = await jobController.jobResultListener.start();
                if (item.isDirectory &&
                    res['file_num'] == (entries.length - 1).toString()) {
                  sendRemoveEmptyDir(item.path, i);
                }
              }
            }
            break;
          }
        } catch (e) {
          print("remove error: $e");
        }
      }
    });
    refresh();
  }

  Future<bool?> showRemoveDialog(
      String title, String content, bool showCheckbox) async {
    return await dialogManager?.show<bool>(
        (setState, Function(bool v) close, context) {
      cancel() => close(false);
      submit() => close(true);
      return CustomAlertDialog(
        title: Row(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            const Icon(Icons.warning_rounded, color: Colors.red),
            Expanded(
              child: Text(title).paddingOnly(
                left: 10,
              ),
            ),
          ],
        ),
        contentBoxConstraints:
            BoxConstraints(minHeight: 100, minWidth: 400, maxWidth: 400),
        content: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(content),
            Text(
              translate("This is irreversible!"),
              style: const TextStyle(
                fontWeight: FontWeight.bold,
                color: Colors.red,
              ),
            ).paddingOnly(top: 20),
            showCheckbox
                ? CheckboxListTile(
                    contentPadding: const EdgeInsets.all(0),
                    dense: true,
                    controlAffinity: ListTileControlAffinity.leading,
                    title: Text(
                      translate("Do this for all conflicts"),
                    ),
                    value: _removeCheckboxRemember,
                    onChanged: (v) {
                      if (v == null) return;
                      setState(() => _removeCheckboxRemember = v);
                    },
                  )
                : const SizedBox.shrink()
          ],
        ),
        actions: [
          dialogButton(
            "Cancel",
            icon: Icon(Icons.close_rounded),
            onPressed: cancel,
            isOutline: true,
          ),
          dialogButton(
            "OK",
            icon: Icon(Icons.done_rounded),
            onPressed: submit,
          ),
        ],
        onSubmit: submit,
        onCancel: cancel,
      );
    }, useAnimation: false);
  }

  void sendRemoveFile(String path, int fileNum) {
    bind.sessionRemoveFile(
        sessionId: sessionId,
        actId: JobController.jobID.next(),
        path: path,
        isRemote: !isLocal,
        fileNum: fileNum);
  }

  void sendRemoveEmptyDir(String path, int fileNum) {
    history.removeWhere((element) => element.contains(path));
    bind.sessionRemoveAllEmptyDirs(
        sessionId: sessionId,
        actId: JobController.jobID.next(),
        path: path,
        isRemote: !isLocal);
  }

  Future<void> createDir(String path) async {
    bind.sessionCreateDir(
        sessionId: sessionId,
        actId: JobController.jobID.next(),
        path: path,
        isRemote: !isLocal);
  }
}

class JobController {
  static final JobID jobID = JobID();
  final jobTable = List<JobProgress>.empty(growable: true).obs;
  final jobResultListener = JobResultListener<Map<String, dynamic>>();
  final GetSessionID getSessionID;
  SessionID get sessionId => getSessionID();

  JobController(this.getSessionID);

  int getJob(int id) {
    return jobTable.indexWhere((element) => element.id == id);
  }

  // JobProgress? getJob(int id) {
  //   return jobTable.firstWhere((element) => element.id == id);
  // }

  // return jobID
  int add(Entry from, bool isRemoteToLocal) {
    final jobID = JobController.jobID.next();
    jobTable.add(JobProgress()
      ..fileName = path.basename(from.path)
      ..jobName = from.path
      ..totalSize = from.size
      ..state = JobState.inProgress
      ..id = jobID
      ..isRemoteToLocal = isRemoteToLocal);
    return jobID;
  }

  void tryUpdateJobProgress(Map<String, dynamic> evt) {
    try {
      int id = int.parse(evt['id']);
      // id = index + 1
      final jobIndex = getJob(id);
      if (jobIndex >= 0 && jobTable.length > jobIndex) {
        final job = jobTable[jobIndex];
        job.fileNum = int.parse(evt['file_num']);
        job.speed = double.parse(evt['speed']);
        job.finishedSize = int.parse(evt['finished_size']);
        debugPrint("update job $id with $evt");
        jobTable.refresh();
      }
    } catch (e) {
      debugPrint("Failed to tryUpdateJobProgress, evt: ${evt.toString()}");
    }
  }

  void jobDone(Map<String, dynamic> evt) async {
    if (jobResultListener.isListening) {
      jobResultListener.complete(evt);
      return;
    }

    int id = int.parse(evt['id']);
    final jobIndex = getJob(id);
    if (jobIndex != -1) {
      final job = jobTable[jobIndex];
      job.finishedSize = job.totalSize;
      job.state = JobState.done;
      job.fileNum = int.parse(evt['file_num']);
      jobTable.refresh();
    }
  }

  void jobError(Map<String, dynamic> evt) {
    final err = evt['err'].toString();
    int jobIndex = getJob(int.parse(evt['id']));
    if (jobIndex != -1) {
      final job = jobTable[jobIndex];
      job.state = JobState.error;
      job.err = err;
      job.fileNum = int.parse(evt['file_num']);
      if (err == "skipped") {
        job.state = JobState.done;
        job.finishedSize = job.totalSize;
      }
      jobTable.refresh();
    }
    debugPrint("jobError $evt");
  }

  Future<void> cancelJob(int id) async {
    await bind.sessionCancelJob(sessionId: sessionId, actId: id);
  }

  void loadLastJob(Map<String, dynamic> evt) {
    debugPrint("load last job: $evt");
    Map<String, dynamic> jobDetail = json.decode(evt['value']);
    // int id = int.parse(jobDetail['id']);
    String remote = jobDetail['remote'];
    String to = jobDetail['to'];
    bool showHidden = jobDetail['show_hidden'];
    int fileNum = jobDetail['file_num'];
    bool isRemote = jobDetail['is_remote'];
    final currJobId = JobController.jobID.next();
    String fileName = path.basename(isRemote ? remote : to);
    var jobProgress = JobProgress()
      ..fileName = fileName
      ..jobName = isRemote ? remote : to
      ..id = currJobId
      ..isRemoteToLocal = isRemote
      ..fileNum = fileNum
      ..remote = remote
      ..to = to
      ..showHidden = showHidden
      ..state = JobState.paused;
    jobTable.add(jobProgress);
    bind.sessionAddJob(
      sessionId: sessionId,
      isRemote: isRemote,
      includeHidden: showHidden,
      actId: currJobId,
      path: isRemote ? remote : to,
      to: isRemote ? to : remote,
      fileNum: fileNum,
    );
  }

  void resumeJob(int jobId) {
    final jobIndex = getJob(jobId);
    if (jobIndex != -1) {
      final job = jobTable[jobIndex];
      bind.sessionResumeJob(
          sessionId: sessionId, actId: job.id, isRemote: job.isRemoteToLocal);
      job.state = JobState.inProgress;
      jobTable.refresh();
    } else {
      debugPrint("jobId $jobId is not exists");
    }
  }

  void updateFolderFiles(Map<String, dynamic> evt) {
    // ret: "{\"id\":1,\"num_entries\":12,\"total_size\":1264822.0}"
    Map<String, dynamic> info = json.decode(evt['info']);
    int id = info['id'];
    int num_entries = info['num_entries'];
    double total_size = info['total_size'];
    final jobIndex = getJob(id);
    if (jobIndex != -1) {
      final job = jobTable[jobIndex];
      job.fileCount = num_entries;
      job.totalSize = total_size.toInt();
      jobTable.refresh();
    }
    debugPrint("update folder files: $info");
  }
}

class JobResultListener<T> {
  Completer<T>? _completer;
  Timer? _timer;
  final int _timeoutSecond = 5;

  bool get isListening => _completer != null;

  clear() {
    if (_completer != null) {
      _timer?.cancel();
      _timer = null;
      _completer!.completeError("Cancel manually");
      _completer = null;
      return;
    }
  }

  Future<T> start() {
    if (_completer != null) return Future.error("Already start listen");
    _completer = Completer();
    _timer = Timer(Duration(seconds: _timeoutSecond), () {
      if (!_completer!.isCompleted) {
        _completer!.completeError("Time out");
      }
      _completer = null;
    });
    return _completer!.future;
  }

  complete(T res) {
    if (_completer != null) {
      _timer?.cancel();
      _timer = null;
      _completer!.complete(res);
      _completer = null;
      return;
    }
  }
}

class FileFetcher {
  // Map<String,Completer<FileDirectory>> localTasks = {}; // now we only use read local dir sync
  Map<String, Completer<FileDirectory>> remoteTasks = {};
  Map<int, Completer<FileDirectory>> readRecursiveTasks = {};

  final GetSessionID getSessionID;
  SessionID get sessionId => getSessionID();

  FileFetcher(this.getSessionID);

  Future<FileDirectory> registerReadTask(bool isLocal, String path) {
    // final jobs = isLocal?localJobs:remoteJobs; // maybe we will use read local dir async later
    final tasks = remoteTasks; // bypass now
    if (tasks.containsKey(path)) {
      throw "Failed to registerReadTask, already have same read job";
    }
    final c = Completer<FileDirectory>();
    tasks[path] = c;

    Timer(Duration(seconds: 2), () {
      tasks.remove(path);
      if (c.isCompleted) return;
      c.completeError("Failed to read dir, timeout");
    });
    return c.future;
  }

  Future<FileDirectory> registerReadRecursiveTask(int actID) {
    final tasks = readRecursiveTasks;
    if (tasks.containsKey(actID)) {
      throw "Failed to registerRemoveTask, already have same ReadRecursive job";
    }
    final c = Completer<FileDirectory>();
    tasks[actID] = c;

    Timer(Duration(seconds: 2), () {
      tasks.remove(actID);
      if (c.isCompleted) return;
      c.completeError("Failed to read dir, timeout");
    });
    return c.future;
  }

  tryCompleteTask(String? msg, String? isLocalStr) {
    if (msg == null || isLocalStr == null) return;
    late final Map<Object, Completer<FileDirectory>> tasks;
    try {
      final fd = FileDirectory.fromJson(jsonDecode(msg));
      if (fd.id > 0) {
        // fd.id > 0 is result for read recursive
        // to-do later,will be better if every fetch use ID,so that there will only one task map for read and recursive read
        tasks = readRecursiveTasks;
        final completer = tasks.remove(fd.id);
        completer?.complete(fd);
      } else if (fd.path.isNotEmpty) {
        // result for normal read dir
        // final jobs = isLocal?localJobs:remoteJobs; // maybe we will use read local dir async later
        tasks = remoteTasks; // bypass now
        final completer = tasks.remove(fd.path);
        completer?.complete(fd);
      }
    } catch (e) {
      debugPrint("tryCompleteJob err: $e");
    }
  }

  Future<FileDirectory> fetchDirectory(
      String path, bool isLocal, bool showHidden) async {
    try {
      if (isLocal) {
        final res = await bind.sessionReadLocalDirSync(
            sessionId: sessionId, path: path, showHidden: showHidden);
        final fd = FileDirectory.fromJson(jsonDecode(res));
        return fd;
      } else {
        await bind.sessionReadRemoteDir(
            sessionId: sessionId, path: path, includeHidden: showHidden);
        return registerReadTask(isLocal, path);
      }
    } catch (e) {
      return Future.error(e);
    }
  }

  Future<FileDirectory> fetchDirectoryRecursive(
      int actID, String path, bool isLocal, bool showHidden) async {
    // TODO test Recursive is show hidden default?
    try {
      await bind.sessionReadDirRecursive(
          sessionId: sessionId,
          actId: actID,
          path: path,
          isRemote: !isLocal,
          showHidden: showHidden);
      return registerReadRecursiveTask(actID);
    } catch (e) {
      return Future.error(e);
    }
  }
}

class FileDirectory {
  List<Entry> entries = [];
  int id = 0;
  String path = "";

  FileDirectory();

  FileDirectory.fromJson(Map<String, dynamic> json) {
    id = json['id'];
    path = json['path'];
    json['entries'].forEach((v) {
      entries.add(Entry.fromJson(v));
    });
  }

  // generate full path for every entry , init sort style if need.
  format(bool isWindows, {SortBy? sort}) {
    for (var entry in entries) {
      entry.path = PathUtil.join(path, entry.name, isWindows);
    }
    if (sort != null) {
      changeSortStyle(sort);
    }
  }

  changeSortStyle(SortBy sort, {bool ascending = true}) {
    entries = _sortList(entries, sort, ascending);
  }

  clear() {
    entries = [];
    id = 0;
    path = "";
  }
}

class Entry {
  int entryType = 4;
  int modifiedTime = 0;
  String name = "";
  String path = "";
  int size = 0;

  Entry();

  Entry.fromJson(Map<String, dynamic> json) {
    entryType = json['entry_type'];
    modifiedTime = json['modified_time'];
    name = json['name'];
    size = json['size'];
  }

  bool get isFile => entryType > 3;

  bool get isDirectory => entryType < 3;

  bool get isDrive => entryType == 3;

  DateTime lastModified() {
    return DateTime.fromMillisecondsSinceEpoch(modifiedTime * 1000);
  }
}

enum JobState { none, inProgress, done, error, paused }

extension JobStateDisplay on JobState {
  String display() {
    switch (this) {
      case JobState.none:
        return translate("Waiting");
      case JobState.inProgress:
        return translate("Transfer file");
      case JobState.done:
        return translate("Finished");
      case JobState.error:
        return translate("Error");
      default:
        return "";
    }
  }
}

class JobProgress {
  JobState state = JobState.none;
  var id = 0;
  var fileNum = 0;
  var speed = 0.0;
  var finishedSize = 0;
  var totalSize = 0;
  var fileCount = 0;
  // [isRemote == true] means [remote -> local]
  // var isRemote = false;
  // to-do use enum
  var isRemoteToLocal = false;
  var jobName = "";
  var fileName = "";
  var remote = "";
  var to = "";
  var showHidden = false;
  var err = "";
  int lastTransferredSize = 0;

  clear() {
    state = JobState.none;
    id = 0;
    fileNum = 0;
    speed = 0;
    finishedSize = 0;
    jobName = "";
    fileName = "";
    fileCount = 0;
    remote = "";
    to = "";
    err = "";
  }

  String display() {
    if (state == JobState.done && err == "skipped") {
      return translate("Skipped");
    }
    return state.display();
  }
}

class _PathStat {
  final String path;
  final DateTime dateTime;

  _PathStat(this.path, this.dateTime);
}

class PathUtil {
  static final windowsContext = path.Context(style: path.Style.windows);
  static final posixContext = path.Context(style: path.Style.posix);

  static String join(String path1, String path2, bool isWindows) {
    final pathUtil = isWindows ? windowsContext : posixContext;
    return pathUtil.join(path1, path2);
  }

  static List<String> split(String path, bool isWindows) {
    final pathUtil = isWindows ? windowsContext : posixContext;
    return pathUtil.split(path);
  }

  static String dirname(String path, bool isWindows) {
    final pathUtil = isWindows ? windowsContext : posixContext;
    return pathUtil.dirname(path);
  }
}

class DirectoryOptions {
  String home;
  bool showHidden;
  bool isWindows;

  DirectoryOptions(
      {this.home = "", this.showHidden = false, this.isWindows = false});

  clear() {
    home = "";
    showHidden = false;
    isWindows = false;
  }
}

class SelectedItems {
  final bool isLocal;
  final items = RxList<Entry>.empty(growable: true);

  SelectedItems({required this.isLocal});

  void add(Entry e) {
    if (e.isDrive) return;
    if (!items.contains(e)) {
      items.add(e);
    }
  }

  void remove(Entry e) {
    items.remove(e);
  }

  void clear() {
    items.clear();
  }

  void selectAll(List<Entry> entries) {
    items.clear();
    items.addAll(entries);
  }

  static bool valid(RxList<Entry> items) {
    if (items.isNotEmpty) {
      // exclude DirDrive type
      return items.any((item) => !item.isDrive);
    }
    return false;
  }
}

// edited from [https://github.com/DevsOnFlutter/file_manager/blob/c1bf7f0225b15bcb86eba602c60acd5c4da90dd8/lib/file_manager.dart#L22]
List<Entry> _sortList(List<Entry> list, SortBy sortType, bool ascending) {
  if (sortType == SortBy.name) {
    // making list of only folders.
    final dirs = list
        .where((element) => element.isDirectory || element.isDrive)
        .toList();
    // sorting folder list by name.
    dirs.sort((a, b) => a.name.toLowerCase().compareTo(b.name.toLowerCase()));

    // making list of only flies.
    final files = list.where((element) => element.isFile).toList();
    // sorting files list by name.
    files.sort((a, b) => a.name.toLowerCase().compareTo(b.name.toLowerCase()));

    // first folders will go to list (if available) then files will go to list.
    return ascending
        ? [...dirs, ...files]
        : [...dirs.reversed.toList(), ...files.reversed.toList()];
  } else if (sortType == SortBy.modified) {
    // making the list of Path & DateTime
    List<_PathStat> pathStat = [];
    for (Entry e in list) {
      pathStat.add(_PathStat(e.name, e.lastModified()));
    }

    // sort _pathStat according to date
    pathStat.sort((b, a) => a.dateTime.compareTo(b.dateTime));

    // sorting [list] according to [_pathStat]
    list.sort((a, b) => pathStat
        .indexWhere((element) => element.path == a.name)
        .compareTo(pathStat.indexWhere((element) => element.path == b.name)));
    return ascending ? list : list.reversed.toList();
  } else if (sortType == SortBy.type) {
    // making list of only folders.
    final dirs = list.where((element) => element.isDirectory).toList();

    // sorting folders by name.
    dirs.sort((a, b) => a.name.toLowerCase().compareTo(b.name.toLowerCase()));

    // making the list of files
    final files = list.where((element) => element.isFile).toList();

    // sorting files list by extension.
    files.sort((a, b) => a.name
        .toLowerCase()
        .split('.')
        .last
        .compareTo(b.name.toLowerCase().split('.').last));
    return ascending
        ? [...dirs, ...files]
        : [...dirs.reversed.toList(), ...files.reversed.toList()];
  } else if (sortType == SortBy.size) {
    // create list of path and size
    Map<String, int> sizeMap = {};
    for (Entry e in list) {
      sizeMap[e.name] = e.size;
    }

    // making list of only folders.
    final dirs = list.where((element) => element.isDirectory).toList();
    // sorting folder list by name.
    dirs.sort((a, b) => a.name.toLowerCase().compareTo(b.name.toLowerCase()));

    // making list of only flies.
    final files = list.where((element) => element.isFile).toList();

    // creating sorted list of [_sizeMapList] by size.
    final List<MapEntry<String, int>> sizeMapList = sizeMap.entries.toList();
    sizeMapList.sort((b, a) => a.value.compareTo(b.value));

    // sort [list] according to [_sizeMapList]
    files.sort((a, b) => sizeMapList
        .indexWhere((element) => element.key == a.name)
        .compareTo(sizeMapList.indexWhere((element) => element.key == b.name)));
    return ascending
        ? [...dirs, ...files]
        : [...dirs.reversed.toList(), ...files.reversed.toList()];
  }
  return [];
}

/// Define a general queue which can accepts different dialog type.
///
/// [Visibility]
/// The `_FileDialogType` and `_DialogEvent` are invisible for other models.
enum FileDialogType { overwrite, unknown }

class _FileDialogEvent extends BaseEvent<FileDialogType, Map<String, dynamic>> {
  WeakReference<FileModel> fileModel;
  bool? _overrideConfirm;
  bool _skip = false;

  _FileDialogEvent(this.fileModel, super.type, super.data);

  void setOverrideConfirm(bool? confirm) {
    _overrideConfirm = confirm;
  }

  void setSkip(bool skip) {
    _skip = skip;
  }

  @override
  EventCallback<Map<String, dynamic>>? findCallback(FileDialogType type) {
    final model = fileModel.target;
    if (model == null) {
      return null;
    }
    switch (type) {
      case FileDialogType.overwrite:
        return (data) async {
          return await model.overrideFileConfirm(data,
              overrideConfirm: _overrideConfirm, skip: _skip);
        };
      default:
        debugPrint("Unknown event type: $type with $data");
        return null;
    }
  }
}

class FileDialogEventLoop
    extends BaseEventLoop<FileDialogType, Map<String, dynamic>> {
  bool? _overrideConfirm;
  bool _skip = false;

  @override
  Future<void> onPreConsume(
      BaseEvent<FileDialogType, Map<String, dynamic>> evt) async {
    var event = evt as _FileDialogEvent;
    event.setOverrideConfirm(_overrideConfirm);
    event.setSkip(_skip);
    debugPrint(
        "FileDialogEventLoop: consuming<jobId: ${evt.data['id']} overrideConfirm: $_overrideConfirm, skip: $_skip>");
  }

  @override
  Future<void> onEventsClear() {
    _overrideConfirm = null;
    _skip = false;
    return super.onEventsClear();
  }

  void setOverrideConfirm(bool? confirm) {
    _overrideConfirm = confirm;
  }

  void setSkip(bool skip) {
    _skip = skip;
  }
}
