import 'dart:convert';
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

typedef OnJobStateChange = void Function(JobState state, JobProgress jp);

// TODO 每个fd设置操作系统属性，不同的操作系统 有不同的文件连字符 封装各类Path功能

class FileModel extends ChangeNotifier {
  var _isLocal = false;
  var _selectMode = false;

  /// 每一个选择的文件或文件夹占用一个 _jobId，file_num是文件夹中的单独文件id
  /// 如
  /// 发送单独一个文件  file_num = 0;
  /// 发送一个文件夹，若文件夹下有3个文件  file_num = 2;
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

  jobDone(Map<String, dynamic> evt) {
    _jobProgress.state = JobState.done;
    refresh();
    notifyListeners();
  }

  jobError(Map<String, dynamic> evt) {
    // TODO
    _jobProgress.clear();
    _jobProgress.state = JobState.error;
    notifyListeners();
  }

  jobReset() {
    _jobProgress.clear();
    notifyListeners();
  }

  tryUpdateDir(String fd, bool isLocal) {
    try {
      final fileDir = FileDirectory.fromJson(jsonDecode(fd), _sortStyle);
      if (isLocal) {
        _currentLocalDir = fileDir;
      } else {
        _currentRemoteDir = fileDir;
      }
      notifyListeners(); // TODO use too early, error occur:setState() or markNeedsBuild() called during build.
    } catch (e) {
      debugPrint("Failed to tryUpdateDir :$fd");
    }
  }

  refresh() {
    openDirectory(_isLocal ? _currentLocalDir.path : _currentRemoteDir.path);
  }

  openDirectory(String path) {
    if (_isLocal) {
      final res = FFI.getByName("read_dir", path);
      tryUpdateDir(res, true);
    } else {
      FFI.setByName("read_remote_dir", path);
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

  removeAction(SelectedItems items) {
    if (items.isLocal == null) {
      debugPrint("Failed to removeFile ,wrong path state");
      return;
    }
    items.items.forEach((entry) {
      _jobId++;
      if (entry.isFile) {
        // TODO dir
        final msg = {
          "id": _jobId.toString(),
          "path": entry.path,
          "file_num": "0",
          "is_remote": (!(items.isLocal!)).toString()
        };
        debugPrint("remove :$msg");
        FFI.setByName("remove_file", jsonEncode(msg));
        // items.remove(entry);
      }
    });
  }

  createDir(String path) {}

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

class FileDirectory {
  List<Entry> entries = [];
  int id = 0;
  String path = "";

  String get parent => p.dirname(path);

  FileDirectory();

  FileDirectory.fromJson(Map<String, dynamic> json, SortBy sort) {
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
