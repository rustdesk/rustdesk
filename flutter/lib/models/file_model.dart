import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:get/get.dart';
import 'package:path/path.dart' as path;

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

class FileModel extends ChangeNotifier {
  /// mobile, current selected page show on mobile screen
  var _isSelectedLocal = false;

  /// mobile, select mode state
  var _selectMode = false;

  final _localOption = DirectoryOption();
  final _remoteOption = DirectoryOption();

  List<String> localHistory = [];
  List<String> remoteHistory = [];

  var _jobId = 0;

  final _jobProgress = JobProgress(); // from rust update

  /// JobTable <jobId, JobProgress>
  final _jobTable = List<JobProgress>.empty(growable: true).obs;

  /// `isLocal` bool
  Function(bool)? onDirChanged;

  RxList<JobProgress> get jobTable => _jobTable;

  bool get isLocal => _isSelectedLocal;

  bool get selectMode => _selectMode;

  JobProgress get jobProgress => _jobProgress;

  JobState get jobState => _jobProgress.state;

  SortBy _sortStyle = SortBy.name;

  SortBy get sortStyle => _sortStyle;

  SortBy _localSortStyle = SortBy.name;

  bool _localSortAscending = true;

  bool _remoteSortAscending = true;

  SortBy _remoteSortStyle = SortBy.name;

  bool get localSortAscending => _localSortAscending;

  SortBy getSortStyle(bool isLocal) {
    return isLocal ? _localSortStyle : _remoteSortStyle;
  }

  bool getSortAscending(bool isLocal) {
    return isLocal ? _localSortAscending : _remoteSortAscending;
  }

  FileDirectory _currentLocalDir = FileDirectory();

  FileDirectory get currentLocalDir => _currentLocalDir;

  FileDirectory _currentRemoteDir = FileDirectory();

  FileDirectory get currentRemoteDir => _currentRemoteDir;

  FileDirectory get currentDir =>
      _isSelectedLocal ? currentLocalDir : currentRemoteDir;

  FileDirectory getCurrentDir(bool isLocal) {
    return isLocal ? currentLocalDir : currentRemoteDir;
  }

  String getCurrentShortPath(bool isLocal) {
    final currentDir = getCurrentDir(isLocal);
    final currentHome = getCurrentHome(isLocal);
    if (currentDir.path.startsWith(currentHome)) {
      var path = currentDir.path.replaceFirst(currentHome, "");
      if (path.isEmpty) return "";
      if (path[0] == "/" || path[0] == "\\") {
        // remove more '/' or '\'
        path = path.replaceFirst(path[0], "");
      }
      return path;
    } else {
      return currentDir.path.replaceFirst(currentHome, "");
    }
  }

  String get currentHome =>
      _isSelectedLocal ? _localOption.home : _remoteOption.home;

  String getCurrentHome(bool isLocal) {
    return isLocal ? _localOption.home : _remoteOption.home;
  }

  int getJob(int id) {
    return jobTable.indexWhere((element) => element.id == id);
  }

  String get currentShortPath {
    if (currentDir.path.startsWith(currentHome)) {
      var path = currentDir.path.replaceFirst(currentHome, "");
      if (path.isEmpty) return "";
      if (path[0] == "/" || path[0] == "\\") {
        // remove more '/' or '\'
        path = path.replaceFirst(path[0], "");
      }
      return path;
    } else {
      return currentDir.path.replaceFirst(currentHome, "");
    }
  }

  String shortPath(bool isLocal) {
    final dir = isLocal ? currentLocalDir : currentRemoteDir;
    if (dir.path.startsWith(currentHome)) {
      var path = dir.path.replaceFirst(currentHome, "");
      if (path.isEmpty) return "";
      if (path[0] == "/" || path[0] == "\\") {
        // remove more '/' or '\'
        path = path.replaceFirst(path[0], "");
      }
      return path;
    } else {
      return dir.path.replaceFirst(currentHome, "");
    }
  }

  bool getCurrentShowHidden([bool? isLocal]) {
    final isLocal_ = isLocal ?? _isSelectedLocal;
    return isLocal_ ? _localOption.showHidden : _remoteOption.showHidden;
  }

  bool getCurrentIsWindows([bool? isLocal]) {
    final isLocal_ = isLocal ?? _isSelectedLocal;
    return isLocal_ ? _localOption.isWindows : _remoteOption.isWindows;
  }

  final _fileFetcher = FileFetcher();

  final _jobResultListener = JobResultListener<Map<String, dynamic>>();

  final WeakReference<FFI> parent;

  FileModel(this.parent);

  toggleSelectMode() {
    if (jobState == JobState.inProgress) {
      return;
    }
    _selectMode = !_selectMode;
    notifyListeners();
  }

  togglePage() {
    _isSelectedLocal = !_isSelectedLocal;
    notifyListeners();
  }

  toggleShowHidden({bool? showHidden, bool? local}) {
    final isLocal = local ?? _isSelectedLocal;
    if (isLocal) {
      _localOption.showHidden = showHidden ?? !_localOption.showHidden;
    } else {
      _remoteOption.showHidden = showHidden ?? !_remoteOption.showHidden;
    }
    refresh(isLocal: local);
  }

  tryUpdateJobProgress(Map<String, dynamic> evt) {
    try {
      int id = int.parse(evt['id']);
      if (!isDesktop) {
        _jobProgress.id = id;
        _jobProgress.fileNum = int.parse(evt['file_num']);
        _jobProgress.speed = double.parse(evt['speed']);
        _jobProgress.finishedSize = int.parse(evt['finished_size']);
      } else {
        // Desktop uses jobTable
        // id = index + 1
        final jobIndex = getJob(id);
        if (jobIndex >= 0 && _jobTable.length > jobIndex) {
          final job = _jobTable[jobIndex];
          job.fileNum = int.parse(evt['file_num']);
          job.speed = double.parse(evt['speed']);
          job.finishedSize = int.parse(evt['finished_size']);
          debugPrint("update job $id with $evt");
        }
      }
      notifyListeners();
    } catch (e) {
      debugPrint("Failed to tryUpdateJobProgress,evt:${evt.toString()}");
    }
  }

  receiveFileDir(Map<String, dynamic> evt) {
    if (evt['is_local'] == "false") {
      // init remote home, the connection will automatic read remote home when established,
      try {
        final fd = FileDirectory.fromJson(jsonDecode(evt['value']));
        fd.format(_remoteOption.isWindows, sort: _sortStyle);
        if (fd.id > 0) {
          final jobIndex = getJob(fd.id);
          if (jobIndex != -1) {
            final job = jobTable[jobIndex];
            var totalSize = 0;
            var fileCount = fd.entries.length;
            for (var element in fd.entries) {
              totalSize += element.size;
            }
            job.totalSize = totalSize;
            job.fileCount = fileCount;
            debugPrint("update receive details:${fd.path}");
          }
        } else if (_remoteOption.home.isEmpty) {
          _remoteOption.home = fd.path;
          debugPrint("init remote home:${fd.path}");
          _currentRemoteDir = fd;
        }
      } catch (e) {
        debugPrint("receiveFileDir err=$e");
      }
    }
    _fileFetcher.tryCompleteTask(evt['value'], evt['is_local']);
    notifyListeners();
  }

  jobDone(Map<String, dynamic> evt) async {
    if (_jobResultListener.isListening) {
      _jobResultListener.complete(evt);
      return;
    }
    if (!isDesktop) {
      _selectMode = false;
      _jobProgress.state = JobState.done;
    } else {
      int id = int.parse(evt['id']);
      final jobIndex = getJob(id);
      if (jobIndex != -1) {
        final job = jobTable[jobIndex];
        job.finishedSize = job.totalSize;
        job.state = JobState.done;
        job.fileNum = int.parse(evt['file_num']);
      }
    }
    await Future.wait([
      refresh(isLocal: false),
      refresh(isLocal: true),
    ]);
  }

  jobError(Map<String, dynamic> evt) {
    final err = evt['err'].toString();
    if (!isDesktop) {
      if (_jobResultListener.isListening) {
        _jobResultListener.complete(evt);
        return;
      }
      _selectMode = false;
      _jobProgress.clear();
      _jobProgress.err = err;
      _jobProgress.state = JobState.error;
      _jobProgress.fileNum = int.parse(evt['file_num']);
      if (err == "skipped") {
        _jobProgress.state = JobState.done;
        _jobProgress.finishedSize = _jobProgress.totalSize;
      }
    } else {
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
      }
    }
    debugPrint("jobError $evt");
    notifyListeners();
  }

  overrideFileConfirm(Map<String, dynamic> evt) async {
    final resp = await showFileConfirmDialog(
        translate("Overwrite"), "${evt['read_path']}", true);
    final id = int.tryParse(evt['id']) ?? 0;
    if (false == resp) {
      final jobIndex = getJob(id);
      if (jobIndex != -1) {
        cancelJob(id);
        final job = jobTable[jobIndex];
        job.state = JobState.done;
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
      bind.sessionSetConfirmOverrideFile(
          id: parent.target?.id ?? "",
          actId: id,
          fileNum: int.parse(evt['file_num']),
          needOverride: need_override,
          remember: fileConfirmCheckboxRemember,
          isUpload: evt['is_upload'] == "true");
    }
  }

  jobReset() {
    _jobProgress.clear();
    notifyListeners();
  }

  onReady() async {
    _localOption.home = await bind.mainGetHomeDir();
    _localOption.showHidden = (await bind.sessionGetPeerOption(
            id: parent.target?.id ?? "", name: "local_show_hidden"))
        .isNotEmpty;
    _localOption.isWindows = Platform.isWindows;

    _remoteOption.showHidden = (await bind.sessionGetPeerOption(
            id: parent.target?.id ?? "", name: "remote_show_hidden"))
        .isNotEmpty;
    _remoteOption.isWindows =
        parent.target?.ffiModel.pi.platform == kPeerPlatformWindows;

    await Future.delayed(Duration(milliseconds: 100));

    final local = (await bind.sessionGetPeerOption(
        id: parent.target?.id ?? "", name: "local_dir"));
    final remote = (await bind.sessionGetPeerOption(
        id: parent.target?.id ?? "", name: "remote_dir"));
    openDirectory(local.isEmpty ? _localOption.home : local, isLocal: true);
    openDirectory(remote.isEmpty ? _remoteOption.home : remote, isLocal: false);
    await Future.delayed(Duration(seconds: 1));
    if (_currentLocalDir.path.isEmpty) {
      openDirectory(_localOption.home, isLocal: true);
    }
    if (_currentRemoteDir.path.isEmpty) {
      openDirectory(_remoteOption.home, isLocal: false);
    }
  }

  Future<void> onClose() async {
    parent.target?.dialogManager.dismissAll();
    jobReset();

    onDirChanged = null;

    // save config
    Map<String, String> msgMap = {};

    msgMap["local_dir"] = _currentLocalDir.path;
    msgMap["local_show_hidden"] = _localOption.showHidden ? "Y" : "";
    msgMap["remote_dir"] = _currentRemoteDir.path;
    msgMap["remote_show_hidden"] = _remoteOption.showHidden ? "Y" : "";
    final id = parent.target?.id ?? "";
    for (final msg in msgMap.entries) {
      await bind.sessionPeerOption(id: id, name: msg.key, value: msg.value);
    }
    _currentLocalDir.clear();
    _currentRemoteDir.clear();
    _localOption.clear();
    _remoteOption.clear();
  }

  Future refresh({bool? isLocal}) async {
    if (isDesktop) {
      isLocal = isLocal ?? _isSelectedLocal;
      isLocal
          ? await openDirectory(currentLocalDir.path, isLocal: isLocal)
          : await openDirectory(currentRemoteDir.path, isLocal: isLocal);
    } else {
      await openDirectory(currentDir.path);
    }
  }

  openDirectory(String path, {bool? isLocal, bool isBack = false}) async {
    isLocal = isLocal ?? _isSelectedLocal;
    if (path == ".") {
      refresh(isLocal: isLocal);
      return;
    }
    if (path == "..") {
      goToParentDirectory(isLocal: isLocal);
      return;
    }
    if (!isBack) {
      pushHistory(isLocal);
    }
    final showHidden = getCurrentShowHidden(isLocal);
    final isWindows = getCurrentIsWindows(isLocal);
    // process /C:\ -> C:\ on Windows
    if (isWindows && path.length > 1 && path[0] == '/') {
      path = path.substring(1);
      if (path[path.length - 1] != '\\') {
        path = "$path\\";
      }
    }
    try {
      final fd = await _fileFetcher.fetchDirectory(path, isLocal, showHidden);
      fd.format(isWindows, sort: _sortStyle);
      if (isLocal) {
        _currentLocalDir = fd;
      } else {
        _currentRemoteDir = fd;
      }
      notifyListeners();
      onDirChanged?.call(isLocal);
    } catch (e) {
      debugPrint("Failed to openDirectory $path: $e");
    }
  }

  Future<FileDirectory> fetchDirectory(path, isLocal, showHidden) async {
    return await _fileFetcher.fetchDirectory(path, isLocal, showHidden);
  }

  void pushHistory(bool isLocal) {
    final history = isLocal ? localHistory : remoteHistory;
    final currPath = isLocal ? currentLocalDir.path : currentRemoteDir.path;
    if (history.isNotEmpty && history.last == currPath) {
      return;
    }
    history.add(currPath);
  }

  goHome({bool? isLocal}) {
    isLocal = isLocal ?? _isSelectedLocal;
    openDirectory(getCurrentHome(isLocal), isLocal: isLocal);
  }

  goBack({bool? isLocal}) {
    isLocal = isLocal ?? _isSelectedLocal;
    final history = isLocal ? localHistory : remoteHistory;
    if (history.isEmpty) return;
    final path = history.removeAt(history.length - 1);
    if (path.isEmpty) return;
    final currPath = isLocal ? currentLocalDir.path : currentRemoteDir.path;
    if (currPath == path) {
      goBack(isLocal: isLocal);
      return;
    }
    openDirectory(path, isLocal: isLocal, isBack: true);
  }

  goToParentDirectory({bool? isLocal}) {
    isLocal = isLocal ?? _isSelectedLocal;
    final isWindows =
        isLocal ? _localOption.isWindows : _remoteOption.isWindows;
    final currDir = isLocal ? currentLocalDir : currentRemoteDir;
    var parent = PathUtil.dirname(currDir.path, isWindows);
    // specially for C:\, D:\, goto '/'
    if (parent == currDir.path && isWindows) {
      openDirectory('/', isLocal: isLocal);
      return;
    }
    openDirectory(parent, isLocal: isLocal);
  }

  /// isRemote only for desktop now, [isRemote == true] means [remote -> local]
  sendFiles(SelectedItems items, {bool isRemote = false}) {
    if (isDesktop) {
      // desktop sendFiles
      final toPath = isRemote ? currentLocalDir.path : currentRemoteDir.path;
      final isWindows =
          isRemote ? _localOption.isWindows : _remoteOption.isWindows;
      final showHidden =
          isRemote ? _localOption.showHidden : _remoteOption.showHidden;
      for (var from in items.items) {
        final jobId = ++_jobId;
        _jobTable.add(JobProgress()
          ..jobName = from.path
          ..totalSize = from.size
          ..state = JobState.inProgress
          ..id = jobId
          ..isRemote = isRemote);
        bind.sessionSendFiles(
            id: '${parent.target?.id}',
            actId: _jobId,
            path: from.path,
            to: PathUtil.join(toPath, from.name, isWindows),
            fileNum: 0,
            includeHidden: showHidden,
            isRemote: isRemote);
        debugPrint(
            "path:${from.path}, toPath:$toPath, to:${PathUtil.join(toPath, from.name, isWindows)}");
      }
    } else {
      if (items.isLocal == null) {
        debugPrint("Failed to sendFiles ,wrong path state");
        return;
      }
      _jobProgress.state = JobState.inProgress;
      final toPath =
          items.isLocal! ? currentRemoteDir.path : currentLocalDir.path;
      final isWindows =
          items.isLocal! ? _localOption.isWindows : _remoteOption.isWindows;
      final showHidden =
          items.isLocal! ? _localOption.showHidden : _remoteOption.showHidden;
      items.items.forEach((from) async {
        _jobId++;
        await bind.sessionSendFiles(
            id: '${parent.target?.id}',
            actId: _jobId,
            path: from.path,
            to: PathUtil.join(toPath, from.name, isWindows),
            fileNum: 0,
            includeHidden: showHidden,
            isRemote: !(items.isLocal!));
      });
    }
  }

  bool removeCheckboxRemember = false;

  removeAction(SelectedItems items, {bool? isLocal}) async {
    isLocal = isLocal ?? _isSelectedLocal;
    removeCheckboxRemember = false;
    if (items.isLocal == null) {
      debugPrint("Failed to removeFile, wrong path state");
      return;
    }
    final isWindows =
        items.isLocal! ? _localOption.isWindows : _remoteOption.isWindows;
    await Future.forEach(items.items, (Entry item) async {
      _jobId++;
      var title = "";
      var content = "";
      late final List<Entry> entries;
      if (item.isFile) {
        title = translate("Are you sure you want to delete this file?");
        content = item.name;
        entries = [item];
      } else if (item.isDirectory) {
        title = translate("Not an empty directory");
        parent.target?.dialogManager.showLoading(translate("Waiting"));
        final fd = await _fileFetcher.fetchDirectoryRecursive(
            _jobId, item.path, items.isLocal!, true);
        if (fd.path.isEmpty) {
          fd.path = item.path;
        }
        fd.format(isWindows);
        parent.target?.dialogManager.dismissAll();
        if (fd.entries.isEmpty) {
          final confirm = await showRemoveDialog(
              translate(
                  "Are you sure you want to delete this empty directory?"),
              item.name,
              false);
          if (confirm == true) {
            sendRemoveEmptyDir(item.path, 0, items.isLocal!);
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
            item.isDirectory);
        try {
          if (confirm == true) {
            sendRemoveFile(entries[i].path, i, items.isLocal!);
            final res = await _jobResultListener.start();
            // handle remove res;
            if (item.isDirectory &&
                res['file_num'] == (entries.length - 1).toString()) {
              sendRemoveEmptyDir(item.path, i, items.isLocal!);
            }
          }
          if (removeCheckboxRemember) {
            if (confirm == true) {
              for (var j = i + 1; j < entries.length; j++) {
                sendRemoveFile(entries[j].path, j, items.isLocal!);
                final res = await _jobResultListener.start();
                if (item.isDirectory &&
                    res['file_num'] == (entries.length - 1).toString()) {
                  sendRemoveEmptyDir(item.path, i, items.isLocal!);
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
    _selectMode = false;
    refresh(isLocal: isLocal);
  }

  Future<bool?> showRemoveDialog(
      String title, String content, bool showCheckbox) async {
    return await parent.target?.dialogManager.show<bool>(
        (setState, Function(bool v) close) {
      cancel() => close(false);
      submit() => close(true);
      return CustomAlertDialog(
        title: Row(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            const Icon(Icons.warning_rounded, color: Colors.red),
            Text(title).paddingOnly(
              left: 10,
            ),
          ],
        ),
        contentBoxConstraints:
            BoxConstraints(minHeight: 80, minWidth: 400, maxWidth: 400),
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
                    value: removeCheckboxRemember,
                    onChanged: (v) {
                      if (v == null) return;
                      setState(() => removeCheckboxRemember = v);
                    },
                  )
                : const SizedBox.shrink(),
            Row(
              mainAxisAlignment: MainAxisAlignment.spaceBetween,
              children: [
                ElevatedButton.icon(
                  icon: Icon(Icons.close_rounded),
                  label: Text(translate("Cancel")),
                  onPressed: cancel,
                ),
                ElevatedButton.icon(
                  icon: Icon(Icons.done_rounded),
                  label: Text(translate("Ok")),
                  onPressed: submit,
                ),
              ],
            ).paddingOnly(top: 20)
          ],
        ),
        onSubmit: submit,
        onCancel: cancel,
      );
    }, useAnimation: false);
  }

  bool fileConfirmCheckboxRemember = false;

  Future<bool?> showFileConfirmDialog(
      String title, String content, bool showCheckbox) async {
    fileConfirmCheckboxRemember = false;
    return await parent.target?.dialogManager.show<bool?>(
        (setState, Function(bool? v) close) {
      cancel() => close(false);
      submit() => close(true);
      return CustomAlertDialog(
        title: Row(
          children: [
            const Icon(Icons.warning, color: Colors.red),
            const SizedBox(width: 20),
            Text(title)
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
          dialogButton("Cancel", onPressed: cancel, isOutline: true),
          dialogButton("Skip", onPressed: () => close(null), isOutline: true),
          dialogButton("OK", onPressed: submit),
        ],
        onSubmit: submit,
        onCancel: cancel,
      );
    }, useAnimation: false);
  }

  sendRemoveFile(String path, int fileNum, bool isLocal) {
    bind.sessionRemoveFile(
        id: '${parent.target?.id}',
        actId: _jobId,
        path: path,
        isRemote: !isLocal,
        fileNum: fileNum);
  }

  sendRemoveEmptyDir(String path, int fileNum, bool isLocal) {
    final history = isLocal ? localHistory : remoteHistory;
    history.removeWhere((element) => element.contains(path));
    bind.sessionRemoveAllEmptyDirs(
        id: '${parent.target?.id}',
        actId: _jobId,
        path: path,
        isRemote: !isLocal);
  }

  createDir(String path, {bool? isLocal}) async {
    isLocal = isLocal ?? this.isLocal;
    _jobId++;
    bind.sessionCreateDir(
        id: '${parent.target?.id}',
        actId: _jobId,
        path: path,
        isRemote: !isLocal);
  }

  cancelJob(int id) async {
    bind.sessionCancelJob(id: '${parent.target?.id}', actId: id);
    jobReset();
  }

  changeSortStyle(SortBy sort, {bool? isLocal, bool ascending = true}) {
    _sortStyle = sort;
    if (isLocal == null) {
      // compatible for mobile logic
      _currentLocalDir.changeSortStyle(sort, ascending: ascending);
      _currentRemoteDir.changeSortStyle(sort, ascending: ascending);
      _localSortStyle = sort;
      _localSortAscending = ascending;
      _remoteSortStyle = sort;
      _remoteSortAscending = ascending;
    } else if (isLocal) {
      _currentLocalDir.changeSortStyle(sort, ascending: ascending);
      _localSortStyle = sort;
      _localSortAscending = ascending;
    } else {
      _currentRemoteDir.changeSortStyle(sort, ascending: ascending);
      _remoteSortStyle = sort;
      _remoteSortAscending = ascending;
    }
    notifyListeners();
  }

  initFileFetcher() {
    _fileFetcher.id = parent.target?.id;
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
    }
    debugPrint("update folder files: $info");
    notifyListeners();
  }

  bool get remoteSortAscending => _remoteSortAscending;

  void loadLastJob(Map<String, dynamic> evt) {
    debugPrint("load last job: $evt");
    Map<String, dynamic> jobDetail = json.decode(evt['value']);
    // int id = int.parse(jobDetail['id']);
    String remote = jobDetail['remote'];
    String to = jobDetail['to'];
    bool showHidden = jobDetail['show_hidden'];
    int fileNum = jobDetail['file_num'];
    bool isRemote = jobDetail['is_remote'];
    final currJobId = _jobId++;
    var jobProgress = JobProgress()
      ..jobName = isRemote ? remote : to
      ..id = currJobId
      ..isRemote = isRemote
      ..fileNum = fileNum
      ..remote = remote
      ..to = to
      ..showHidden = showHidden
      ..state = JobState.paused;
    jobTable.add(jobProgress);
    bind.sessionAddJob(
      id: '${parent.target?.id}',
      isRemote: isRemote,
      includeHidden: showHidden,
      actId: currJobId,
      path: isRemote ? remote : to,
      to: isRemote ? to : remote,
      fileNum: fileNum,
    );
  }

  resumeJob(int jobId) {
    final jobIndex = getJob(jobId);
    if (jobIndex != -1) {
      final job = jobTable[jobIndex];
      bind.sessionResumeJob(
          id: '${parent.target?.id}', actId: job.id, isRemote: job.isRemote);
      job.state = JobState.inProgress;
    } else {
      debugPrint("jobId $jobId is not exists");
    }
    notifyListeners();
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

  String? id;

  // if id == null, means to fetch global FFI
  FFI get _ffi => ffi(id ?? "");

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
      c.completeError("Failed to read dir,timeout");
    });
    return c.future;
  }

  Future<FileDirectory> registerReadRecursiveTask(int id) {
    final tasks = readRecursiveTasks;
    if (tasks.containsKey(id)) {
      throw "Failed to registerRemoveTask, already have same ReadRecursive job";
    }
    final c = Completer<FileDirectory>();
    tasks[id] = c;

    Timer(Duration(seconds: 2), () {
      tasks.remove(id);
      if (c.isCompleted) return;
      c.completeError("Failed to read dir,timeout");
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
      debugPrint("tryCompleteJob err :$e");
    }
  }

  Future<FileDirectory> fetchDirectory(
      String path, bool isLocal, bool showHidden) async {
    try {
      if (isLocal) {
        final res = await bind.sessionReadLocalDirSync(
            id: id ?? "", path: path, showHidden: showHidden);
        final fd = FileDirectory.fromJson(jsonDecode(res));
        return fd;
      } else {
        await bind.sessionReadRemoteDir(
            id: id ?? "", path: path, includeHidden: showHidden);
        return registerReadTask(isLocal, path);
      }
    } catch (e) {
      return Future.error(e);
    }
  }

  Future<FileDirectory> fetchDirectoryRecursive(
      int id, String path, bool isLocal, bool showHidden) async {
    // TODO test Recursive is show hidden default?
    try {
      await bind.sessionReadDirRecursive(
          id: _ffi.id,
          actId: id,
          path: path,
          isRemote: !isLocal,
          showHidden: showHidden);
      return registerReadRecursiveTask(id);
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
        return translate("Transfer File");
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
  var isRemote = false;
  var jobName = "";
  var remote = "";
  var to = "";
  var showHidden = false;
  var err = "";

  clear() {
    state = JobState.none;
    id = 0;
    fileNum = 0;
    speed = 0;
    finishedSize = 0;
    jobName = "";
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

class DirectoryOption {
  String home;
  bool showHidden;
  bool isWindows;

  DirectoryOption(
      {this.home = "", this.showHidden = false, this.isWindows = false});

  clear() {
    home = "";
    showHidden = false;
    isWindows = false;
  }
}

class SelectedItems {
  bool? _isLocal;
  final List<Entry> _items = [];

  List<Entry> get items => _items;

  int get length => _items.length;

  bool? get isLocal => _isLocal;

  add(bool isLocal, Entry e) {
    if (e.isDrive) return;
    _isLocal ??= isLocal;
    if (_isLocal != null && _isLocal != isLocal) {
      return;
    }
    if (!_items.contains(e)) {
      _items.add(e);
    }
  }

  bool contains(Entry e) {
    return _items.contains(e);
  }

  remove(Entry e) {
    _items.remove(e);
    if (_items.isEmpty) {
      _isLocal = null;
    }
  }

  bool isOtherPage(bool currentIsLocal) {
    if (_isLocal == null) {
      return false;
    } else {
      return _isLocal != currentIsLocal;
    }
  }

  clear() {
    _items.clear();
    _isLocal = null;
  }

  void selectAll(List<Entry> entries) {
    _items.clear();
    _items.addAll(entries);
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
