import 'dart:async';
import 'dart:convert';
import 'package:flutter_easyloading/flutter_easyloading.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/pages/file_manager_page.dart';
import 'package:path/path.dart' as p;
import 'package:flutter/material.dart';
import 'package:path/path.dart' as Path;

import 'model.dart';

enum SortBy { name, type, date, size }

// enum FileType {
//   Dir = 0,
//   DirLink = 2,
//   DirDrive = 3,
//   File = 4,
//   FileLink = 5,
// }

class RemoveCompleter {}

typedef OnJobStateChange = void Function(JobState state, JobProgress jp);

class FileModel extends ChangeNotifier {
  // TODO 添加 dispose 退出页面的时候清理数据以及尚未完成的任务和对话框
  var _isLocal = false;
  var _selectMode = false;

  /// 每一个选择的文件或文件夹占用一个 _jobId，file_num是文件夹中的单独文件id
  /// 如
  /// 发送单独一个文件  file_num = 0;
  /// 发送一个文件夹，若文件夹下有3个文件  最后一个文件的 file_num = 2;
  var _jobId = 0;

  var _jobProgress = JobProgress(); // from rust update

  bool get isLocal => _isLocal;

  bool get selectMode => _selectMode;

  JobProgress get jobProgress => _jobProgress;

  JobState get jobState => _jobProgress.state;

  SortBy _sortStyle = SortBy.name;

  SortBy get sortStyle => _sortStyle;

  FileDirectory _currentLocalDir = FileDirectory();

  FileDirectory get currentLocalDir => _currentLocalDir;

  FileDirectory _currentRemoteDir = FileDirectory();

  FileDirectory get currentRemoteDir => _currentRemoteDir;

  FileDirectory get currentDir => _isLocal ? currentLocalDir : currentRemoteDir;

  final _fileFetcher = FileFetcher();

  final _jobResultListener = JobResultListener<Map<String, dynamic>>();

  toggleSelectMode() {
    _selectMode = !_selectMode;
    notifyListeners();
  }

  togglePage() {
    _isLocal = !_isLocal;
    notifyListeners();
  }

  tryUpdateJobProgress(Map<String, dynamic> evt) {
    try {
      int id = int.parse(evt['id']);
      _jobProgress.id = id;
      _jobProgress.fileNum = int.parse(evt['file_num']);
      _jobProgress.speed = double.parse(evt['speed']);
      _jobProgress.finishedSize = int.parse(evt['finished_size']);
      debugPrint("_jobProgress update:${_jobProgress.toString()}");
      notifyListeners();
    } catch (e) {
      debugPrint("Failed to tryUpdateJobProgress,evt:${evt.toString()}");
    }
  }

  receiveFileDir(Map<String, dynamic> evt) {
    _fileFetcher.tryCompleteTask(evt['value'], evt['is_local']);
  }

  // job 类型 复制结束 删除结束
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

    // TODO
    _selectMode = false;
    _jobProgress.clear();
    _jobProgress.state = JobState.error;
    notifyListeners();
  }

  jobReset() {
    _jobProgress.clear();
    notifyListeners();
  }

  onReady() {
    openDirectory(FFI.getByName("get_home_dir"), isLocal: true);
    openDirectory(FFI.ffiModel.pi.homeDir, isLocal: false);
  }

  refresh() {
    openDirectory(_isLocal ? _currentLocalDir.path : _currentRemoteDir.path);
  }

  openDirectory(String path, {bool? isLocal}) async {
    isLocal = isLocal ?? _isLocal;
    try {
      final fd = await _fileFetcher.fetchDirectory(path, isLocal);
      fd.changeSortStyle(_sortStyle);
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

  goToParentDirectory() {
    final fd = _isLocal ? _currentLocalDir : _currentRemoteDir;
    openDirectory(fd.parent);
  }

  sendFiles(SelectedItems items) {
    if (items.isLocal == null) {
      debugPrint("Failed to sendFiles ,wrong path state");
      return;
    }
    _jobProgress.state = JobState.inProgress;
    final toPath =
        items.isLocal! ? currentRemoteDir.path : currentLocalDir.path;
    items.items.forEach((from) {
      _jobId++;
      final msg = {
        "id": _jobId.toString(),
        "path": from.path,
        "to": Path.join(toPath, from.name),
        "show_hidden": "false", // TODO showHidden
        "is_remote": (!(items.isLocal!)).toString() // 指from的位置而不是to的位置
      };
      FFI.setByName("send_files", jsonEncode(msg));
    });
  }

  bool removeCheckboxRemember = false;

  removeAction(SelectedItems items) async {
    removeCheckboxRemember = false;
    if (items.isLocal == null) {
      debugPrint("Failed to removeFile ,wrong path state");
      return;
    }
    await Future.forEach(items.items, (Entry item) async {
      _jobId++;
      var title = "";
      var content = "";
      late final List<Entry> entries;
      if (item.isFile) {
        title = "是否永久删除文件";
        content = "${item.name}";
        entries = [item];
      } else if (item.isDirectory) {
        title = "这不是一个空文件夹";
        showLoading("正在读取...");
        final fd = await _fileFetcher.fetchDirectoryRecursive(
            _jobId, item.path, items.isLocal!);
        EasyLoading.dismiss();
        // 空文件夹
        if(fd.entries.isEmpty){
          final confirm = await showRemoveDialog("是否删除空文件夹",item.name,false);
          if(confirm == true){
            sendRemoveEmptyDir(item.path, 0, items.isLocal!);
          }
          return;
        }

        debugPrint("removeDirAllIntent res:${fd.id}");
        entries = fd.entries;
      } else {
        debugPrint("none : ${item.toString()}");
        entries = [];
      }

      for (var i = 0; i < entries.length; i++) {
        final dirShow = item.isDirectory?"是否删除文件夹下的文件?\n":"";
        final count = entries.length>1?"第 ${i + 1}/${entries.length} 项":"";
        content = dirShow + "$count \n${entries[i].path}";
        final confirm = await showRemoveDialog(title,content,item.isDirectory);
        debugPrint("已选择:$confirm");
        try {
          if (confirm == true) {
            sendRemoveFile(entries[i].path, i, items.isLocal!);
            final res = await _jobResultListener.start();
            debugPrint("remove got res ${res.toString()}");
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
                debugPrint("remove got res ${res.toString()}");
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
    refresh();
  }

  Future<bool?> showRemoveDialog(String title,String content,bool showCheckbox) async {
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
                  Text("此操作不可逆!",style: TextStyle(fontWeight: FontWeight.bold)),
                  showCheckbox?
                  CheckboxListTile(
                    contentPadding: const EdgeInsets.all(0),
                    dense: true,
                    controlAffinity: ListTileControlAffinity.leading,
                    title: Text(
                      "应用于文件夹下所有文件",
                    ),
                    value: removeCheckboxRemember,
                    onChanged: (v) {
                      if (v == null) return;
                      setState(() => removeCheckboxRemember = v);
                    },
                  ):SizedBox.shrink()
                ]),
            actions: [
              TextButton(
                  style: flatButtonStyle,
                  onPressed: () => close(true), child: Text("Yes")),
              TextButton(
                  style: flatButtonStyle,
                  onPressed: () => close(false), child: Text("No"))
            ]));
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
    _jobId ++;
    final msg = {
      "id": _jobId.toString(),
      "path": path,
      "is_remote": (!isLocal).toString()
    };
    FFI.setByName("create_dir",jsonEncode(msg));
  }
  
  cancelJob(int id){
    
  }

  changeSortStyle(SortBy sort) {
    _sortStyle = sort;
    _currentLocalDir.changeSortStyle(sort);
    _currentRemoteDir.changeSortStyle(sort);
    notifyListeners();
  }

  void clear() {
    _currentLocalDir.clear();
    _currentRemoteDir.clear();
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
      if (c.isCompleted) return; // 计时器加入map
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
      if (c.isCompleted) return; // 计时器加入map
      c.completeError("Failed to read dir,timeout");
    });
    return c.future;
  }

  tryCompleteTask(String? msg, String? isLocalStr) {
    debugPrint("tryCompleteTask : $msg");
    if (msg == null || isLocalStr == null) return;
    late final isLocal;
    late final tasks;
    if (isLocalStr == "true") {
      isLocal = true;
    } else if (isLocalStr == "false") {
      isLocal = false;
    } else {
      return;
    }
    try {
      final fd = FileDirectory.fromJson(jsonDecode(msg));
      if (fd.id > 0) {
        // fd.id > 0 is result for read recursive
        // TODO later,will be better if every fetch use ID,so that there will only one task map for read and recursive read
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

  Future<FileDirectory> fetchDirectory(String path, bool isLocal) async {
    debugPrint("fetch :$path");
    try {
      if (isLocal) {
        final res = FFI.getByName("read_dir", path);
        final fd = FileDirectory.fromJson(jsonDecode(res));
        return fd;
      } else {
        FFI.setByName("read_remote_dir", path);
        return registerReadTask(isLocal, path);
      }
    } catch (e) {
      return Future.error(e);
    }
  }

  Future<FileDirectory> fetchDirectoryRecursive(
      int id, String path, bool isLocal) async {
    debugPrint("fetchDirectoryRecursive id:$id , path:$path");
    try {
      final msg = {
        "id": id.toString(),
        "path": path,
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

  String get parent => p.dirname(path);

  FileDirectory();

  FileDirectory.fromJsonWithSort(Map<String, dynamic> json, SortBy sort) {
    id = json['id'];
    path = json['path'];
    if (json['entries'] != null) {
      entries = <Entry>[];
      json['entries'].forEach((v) {
        entries.add(new Entry.fromJsonWithPath(v, path));
      });
      entries = _sortList(entries, sort);
    }
  }

  FileDirectory.fromJson(Map<String, dynamic> json) {
    id = json['id'];
    path = json['path'];
    if (json['entries'] != null) {
      entries = <Entry>[];
      json['entries'].forEach((v) {
        entries.add(new Entry.fromJsonWithPath(v, path));
      });
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

  Entry.fromJsonWithPath(Map<String, dynamic> json, String parent) {
    entryType = json['entry_type'];
    modifiedTime = json['modified_time'];
    name = json['name'];
    size = json['size'];
    path = Path.join(parent, name);
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

// code from file_manager pkg after edit
List<Entry> _sortList(List<Entry> list, SortBy sortType) {
  if (sortType == SortBy.name) {
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
  } else if (sortType == SortBy.date) {
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
    return [...dirs, ...files];
  } else if (sortType == SortBy.size) {
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
