import 'dart:async';
import 'dart:convert';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/pages/file_manager_page.dart';
import 'package:flutter/material.dart';
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';
import 'package:path/path.dart' as Path;

import 'model.dart';

enum SortBy { Name, Type, Modified, Size }

class FileModel extends ChangeNotifier {
  var _isLocal = false;
  var _selectMode = false;

  var _localOption = DirectoryOption();
  var _remoteOption = DirectoryOption();

  var _jobId = 0;

  var _jobProgress = JobProgress(); // from rust update

  bool get isLocal => _isLocal;

  bool get selectMode => _selectMode;

  JobProgress get jobProgress => _jobProgress;

  JobState get jobState => _jobProgress.state;

  SortBy _sortStyle = SortBy.Name;

  SortBy get sortStyle => _sortStyle;

  FileDirectory _currentLocalDir = FileDirectory();

  FileDirectory get currentLocalDir => _currentLocalDir;

  FileDirectory _currentRemoteDir = FileDirectory();

  FileDirectory get currentRemoteDir => _currentRemoteDir;

  FileDirectory get currentDir => _isLocal ? currentLocalDir : currentRemoteDir;

  String get currentHome => _isLocal ? _localOption.home : _remoteOption.home;

  String get currentShortPath {
    if (currentDir.path.startsWith(currentHome)) {
      var path = currentDir.path.replaceFirst(currentHome, "");
      if (path.length == 0) return "";
      if (path[0] == "/" || path[0] == "\\") {
        // remove more '/' or '\'
        path = path.replaceFirst(path[0], "");
      }
      return path;
    } else {
      return currentDir.path.replaceFirst(currentHome, "");
    }
  }

  bool get currentShowHidden =>
      _isLocal ? _localOption.showHidden : _remoteOption.showHidden;

  bool get currentIsWindows =>
      _isLocal ? _localOption.isWindows : _remoteOption.isWindows;

  final _fileFetcher = FileFetcher();

  final _jobResultListener = JobResultListener<Map<String, dynamic>>();

  toggleSelectMode() {
    if (jobState == JobState.inProgress) {
      return;
    }
    _selectMode = !_selectMode;
    notifyListeners();
  }

  togglePage() {
    _isLocal = !_isLocal;
    notifyListeners();
  }

  toggleShowHidden({bool? showHidden, bool? local}) {
    final isLocal = local ?? _isLocal;
    if (isLocal) {
      _localOption.showHidden = showHidden ?? !_localOption.showHidden;
    } else {
      _remoteOption.showHidden = showHidden ?? !_remoteOption.showHidden;
    }
    refresh();
  }

  tryUpdateJobProgress(Map<String, dynamic> evt) {
    try {
      int id = int.parse(evt['id']);
      _jobProgress.id = id;
      _jobProgress.fileNum = int.parse(evt['file_num']);
      _jobProgress.speed = double.parse(evt['speed']);
      _jobProgress.finishedSize = int.parse(evt['finished_size']);
      notifyListeners();
    } catch (e) {
      debugPrint("Failed to tryUpdateJobProgress,evt:${evt.toString()}");
    }
  }

  receiveFileDir(Map<String, dynamic> evt) {
    if (_remoteOption.home.isEmpty && evt['is_local'] == "false") {
      // init remote home, the connection will automatic read remote home when established,
      try {
        final fd = FileDirectory.fromJson(jsonDecode(evt['value']));
        fd.format(_remoteOption.isWindows, sort: _sortStyle);
        _remoteOption.home = fd.path;
        debugPrint("init remote home:${fd.path}");
        _currentRemoteDir = fd;
        notifyListeners();
        return;
      } finally {}
    }
    _fileFetcher.tryCompleteTask(evt['value'], evt['is_local']);
  }

  jobDone(Map<String, dynamic> evt) {
    if (_jobResultListener.isListening) {
      _jobResultListener.complete(evt);
      return;
    }
    _selectMode = false;
    _jobProgress.state = JobState.done;
    refresh();
  }

  jobError(Map<String, dynamic> evt) {
    if (_jobResultListener.isListening) {
      _jobResultListener.complete(evt);
      return;
    }

    debugPrint("jobError $evt");
    _selectMode = false;
    _jobProgress.clear();
    _jobProgress.state = JobState.error;
    notifyListeners();
  }

  overrideFileConfirm(Map<String, dynamic> evt) async {
    final resp = await showFileConfirmDialog(
        translate("Overwrite"), "${evt['read_path']}", true);
    if (false == resp) {
      cancelJob(int.tryParse(evt['id']) ?? 0);
    } else {
      var msg = Map()
        ..['id'] = evt['id']
        ..['file_num'] = evt['file_num']
        ..['is_upload'] = evt['is_upload']
        ..['remember'] = fileConfirmCheckboxRemember.toString();
      if (resp == null) {
        // skip
        msg['need_override'] = 'false';
      } else {
        // overwrite
        msg['need_override'] = 'true';
      }
      FFI.setByName("set_confirm_override_file", jsonEncode(msg));
    }
  }

  jobReset() {
    _jobProgress.clear();
    notifyListeners();
  }

  onReady() async {
    _localOption.home = FFI.getByName("get_home_dir");
    _localOption.showHidden =
        FFI.getByName("peer_option", "local_show_hidden").isNotEmpty;

    _remoteOption.showHidden =
        FFI.getByName("peer_option", "remote_show_hidden").isNotEmpty;
    _remoteOption.isWindows = FFI.ffiModel.pi.platform == "Windows";

    debugPrint("remote platform: ${FFI.ffiModel.pi.platform}");

    await Future.delayed(Duration(milliseconds: 100));

    final local = FFI.getByName("peer_option", "local_dir");
    final remote = FFI.getByName("peer_option", "remote_dir");
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

  onClose() {
    SmartDialog.dismiss();
    jobReset();

    // save config
    Map<String, String> msg = Map();

    msg["name"] = "local_dir";
    msg["value"] = _currentLocalDir.path;
    FFI.setByName('peer_option', jsonEncode(msg));

    msg["name"] = "local_show_hidden";
    msg["value"] = _localOption.showHidden ? "Y" : "";
    FFI.setByName('peer_option', jsonEncode(msg));

    msg["name"] = "remote_dir";
    msg["value"] = _currentRemoteDir.path;
    FFI.setByName('peer_option', jsonEncode(msg));

    msg["name"] = "remote_show_hidden";
    msg["value"] = _remoteOption.showHidden ? "Y" : "";
    FFI.setByName('peer_option', jsonEncode(msg));
    _currentLocalDir.clear();
    _currentRemoteDir.clear();
    _localOption.clear();
    _remoteOption.clear();
  }

  refresh() {
    openDirectory(currentDir.path);
  }

  openDirectory(String path, {bool? isLocal}) async {
    isLocal = isLocal ?? _isLocal;
    final showHidden =
        isLocal ? _localOption.showHidden : _remoteOption.showHidden;
    final isWindows =
        isLocal ? _localOption.isWindows : _remoteOption.isWindows;
    try {
      final fd = await _fileFetcher.fetchDirectory(path, isLocal, showHidden);
      fd.format(isWindows, sort: _sortStyle);
      if (isLocal) {
        _currentLocalDir = fd;
      } else {
        _currentRemoteDir = fd;
      }
      notifyListeners();
    } catch (e) {
      debugPrint("Failed to openDirectory :$e");
    }
  }

  goHome() {
    openDirectory(currentHome);
  }

  goToParentDirectory() {
    final parent = PathUtil.dirname(currentDir.path, currentIsWindows);
    openDirectory(parent);
  }

  sendFiles(SelectedItems items) {
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
    items.items.forEach((from) {
      _jobId++;
      final msg = {
        "id": _jobId.toString(),
        "path": from.path,
        "to": PathUtil.join(toPath, from.name, isWindows),
        "file_num": "0",
        "show_hidden": showHidden.toString(),
        "is_remote": (!(items.isLocal!)).toString()
      };
      FFI.setByName("send_files", jsonEncode(msg));
    });
  }

  bool removeCheckboxRemember = false;

  removeAction(SelectedItems items) async {
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
        content = "${item.name}";
        entries = [item];
      } else if (item.isDirectory) {
        title = translate("Not an empty directory");
        showLoading(translate("Waiting"));
        final fd = await _fileFetcher.fetchDirectoryRecursive(
            _jobId, item.path, items.isLocal!, true);
        if (fd.path.isEmpty) {
          fd.path = item.path;
        }
        fd.format(isWindows);
        SmartDialog.dismiss();
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
        content = dirShow + "$count \n${entries[i].path}";
        final confirm =
            await showRemoveDialog(title, content, item.isDirectory);
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
        } catch (e) {}
      }
    });
    _selectMode = false;
    refresh();
  }

  Future<bool?> showRemoveDialog(
      String title, String content, bool showCheckbox) async {
    return await DialogManager.show<bool>(
        (setState, Function(bool v) close) => CustomAlertDialog(
                title: Row(
                  children: [
                    Icon(Icons.warning, color: Colors.red),
                    SizedBox(width: 20),
                    Text(title)
                  ],
                ),
                content: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Text(content),
                      SizedBox(height: 5),
                      Text(translate("This is irreversible!"),
                          style: TextStyle(fontWeight: FontWeight.bold)),
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
                          : SizedBox.shrink()
                    ]),
                actions: [
                  TextButton(
                      style: flatButtonStyle,
                      onPressed: () => close(false),
                      child: Text(translate("Cancel"))),
                  TextButton(
                      style: flatButtonStyle,
                      onPressed: () => close(true),
                      child: Text(translate("OK"))),
                ]),
        useAnimation: false);
  }

  bool fileConfirmCheckboxRemember = false;

  Future<bool?> showFileConfirmDialog(
      String title, String content, bool showCheckbox) async {
    fileConfirmCheckboxRemember = false;
    return await DialogManager.show<bool?>(
        (setState, Function(bool? v) close) => CustomAlertDialog(
                title: Row(
                  children: [
                    Icon(Icons.warning, color: Colors.red),
                    SizedBox(width: 20),
                    Text(title)
                  ],
                ),
                content: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Text(
                          translate(
                              "This file exists, skip or overwrite this file?"),
                          style: TextStyle(fontWeight: FontWeight.bold)),
                      SizedBox(height: 5),
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
                          : SizedBox.shrink()
                    ]),
                actions: [
                  TextButton(
                      style: flatButtonStyle,
                      onPressed: () => close(false),
                      child: Text(translate("Cancel"))),
                  TextButton(
                      style: flatButtonStyle,
                      onPressed: () => close(null),
                      child: Text(translate("Skip"))),
                  TextButton(
                      style: flatButtonStyle,
                      onPressed: () => close(true),
                      child: Text(translate("OK"))),
                ]),
        useAnimation: false);
  }

  sendRemoveFile(String path, int fileNum, bool isLocal) {
    final msg = {
      "id": _jobId.toString(),
      "path": path,
      "file_num": fileNum.toString(),
      "is_remote": (!(isLocal)).toString()
    };
    FFI.setByName("remove_file", jsonEncode(msg));
  }

  sendRemoveEmptyDir(String path, int fileNum, bool isLocal) {
    final msg = {
      "id": _jobId.toString(),
      "path": path,
      "is_remote": (!isLocal).toString()
    };
    FFI.setByName("remove_all_empty_dirs", jsonEncode(msg));
  }

  createDir(String path) {
    _jobId++;
    final msg = {
      "id": _jobId.toString(),
      "path": path,
      "is_remote": (!isLocal).toString()
    };
    FFI.setByName("create_dir", jsonEncode(msg));
  }

  cancelJob(int id) {
    FFI.setByName("cancel_job", id.toString());
    jobReset();
  }

  changeSortStyle(SortBy sort) {
    _sortStyle = sort;
    _currentLocalDir.changeSortStyle(sort);
    _currentRemoteDir.changeSortStyle(sort);
    notifyListeners();
  }
}

class JobResultListener<T> {
  Completer<T>? _completer;
  Timer? _timer;
  int _timeoutSecond = 5;

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
  // Map<String,Completer<FileDirectory>> localTasks = Map(); // now we only use read local dir sync
  Map<String, Completer<FileDirectory>> remoteTasks = Map();
  Map<int, Completer<FileDirectory>> readRecursiveTasks = Map();

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
    late final tasks;
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
      final msg = {"path": path, "show_hidden": showHidden.toString()};
      if (isLocal) {
        final res = FFI.getByName("read_local_dir_sync", jsonEncode(msg));
        final fd = FileDirectory.fromJson(jsonDecode(res));
        return fd;
      } else {
        FFI.setByName("read_remote_dir", jsonEncode(msg));
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
      final msg = {
        "id": id.toString(),
        "path": path,
        "show_hidden": showHidden.toString(),
        "is_remote": (!isLocal).toString()
      };
      FFI.setByName("read_dir_recursive", jsonEncode(msg));
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
      entries.add(new Entry.fromJson(v));
    });
  }

  // generate full path for every entry , init sort style if need.
  format(bool isWindows, {SortBy? sort}) {
    entries.forEach((entry) {
      entry.path = PathUtil.join(path, entry.name, isWindows);
    });
    if (sort != null) {
      changeSortStyle(sort);
    }
  }

  changeSortStyle(SortBy sort) {
    entries = _sortList(entries, sort);
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

  bool get isDirectory => entryType <= 3;

  DateTime lastModified() {
    return DateTime.fromMillisecondsSinceEpoch(modifiedTime * 1000);
  }
}

enum JobState { none, inProgress, done, error }

class JobProgress {
  JobState state = JobState.none;
  var id = 0;
  var fileNum = 0;
  var speed = 0.0;
  var finishedSize = 0;

  clear() {
    state = JobState.none;
    id = 0;
    fileNum = 0;
    speed = 0;
    finishedSize = 0;
  }
}

class _PathStat {
  final String path;
  final DateTime dateTime;

  _PathStat(this.path, this.dateTime);
}

class PathUtil {
  static final windowsContext = Path.Context(style: Path.Style.windows);
  static final posixContext = Path.Context(style: Path.Style.posix);

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

// code from file_manager pkg after edit
List<Entry> _sortList(List<Entry> list, SortBy sortType) {
  if (sortType == SortBy.Name) {
    // making list of only folders.
    final dirs = list.where((element) => element.isDirectory).toList();
    // sorting folder list by name.
    dirs.sort((a, b) => a.name.toLowerCase().compareTo(b.name.toLowerCase()));

    // making list of only flies.
    final files = list.where((element) => element.isFile).toList();
    // sorting files list by name.
    files.sort((a, b) => a.name.toLowerCase().compareTo(b.name.toLowerCase()));

    // first folders will go to list (if available) then files will go to list.
    return [...dirs, ...files];
  } else if (sortType == SortBy.Modified) {
    // making the list of Path & DateTime
    List<_PathStat> _pathStat = [];
    for (Entry e in list) {
      _pathStat.add(_PathStat(e.name, e.lastModified()));
    }

    // sort _pathStat according to date
    _pathStat.sort((b, a) => a.dateTime.compareTo(b.dateTime));

    // sorting [list] according to [_pathStat]
    list.sort((a, b) => _pathStat
        .indexWhere((element) => element.path == a.name)
        .compareTo(_pathStat.indexWhere((element) => element.path == b.name)));
    return list;
  } else if (sortType == SortBy.Type) {
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
    return [...dirs, ...files];
  } else if (sortType == SortBy.Size) {
    // create list of path and size
    Map<String, int> _sizeMap = {};
    for (Entry e in list) {
      _sizeMap[e.name] = e.size;
    }

    // making list of only folders.
    final dirs = list.where((element) => element.isDirectory).toList();
    // sorting folder list by name.
    dirs.sort((a, b) => a.name.toLowerCase().compareTo(b.name.toLowerCase()));

    // making list of only flies.
    final files = list.where((element) => element.isFile).toList();

    // creating sorted list of [_sizeMapList] by size.
    final List<MapEntry<String, int>> _sizeMapList = _sizeMap.entries.toList();
    _sizeMapList.sort((b, a) => a.value.compareTo(b.value));

    // sort [list] according to [_sizeMapList]
    files.sort((a, b) => _sizeMapList
        .indexWhere((element) => element.key == a.name)
        .compareTo(
            _sizeMapList.indexWhere((element) => element.key == b.name)));
    return [...dirs, ...files];
  }
  return [];
}
