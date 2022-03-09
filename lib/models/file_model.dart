import 'dart:convert';
import 'package:path/path.dart' as p;
import 'package:flutter/material.dart';
import 'model.dart';

enum SortBy { name, type, date, size }

// enum FileType {
//   Dir = 0,
//   DirLink = 2,
//   DirDrive = 3,
//   File = 4,
//   FileLink = 5,
// }

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
        entries.add(new Entry.fromJson(v));
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

// TODO 使用工厂单例模式

class FileModel extends ChangeNotifier {
  var _jobCount = 0;

  SortBy _sortStyle = SortBy.name;

  SortBy get sortStyle => _sortStyle;

  FileDirectory _currentLocalDir = FileDirectory();

  FileDirectory get currentLocalDir => _currentLocalDir;

  FileDirectory _currentRemoteDir = FileDirectory();

  FileDirectory get currentRemoteDir => _currentRemoteDir;

  tryUpdateDir(String fd, bool isLocal) {
    try {
      final fileDir = FileDirectory.fromJson(jsonDecode(fd), _sortStyle);
      if (isLocal) {
        _currentLocalDir = fileDir;
      } else {
        _currentRemoteDir = fileDir;
      }
      notifyListeners();
    } catch (e) {
      debugPrint("tryUpdateDir fail:$fd");
    }
  }

  refresh(bool isLocal){
    openDirectory(isLocal?_currentLocalDir.path:_currentRemoteDir.path,isLocal);
  }

  openDirectory(String path, bool isLocal) {
    if (isLocal) {
      final res = FFI.getByName("read_dir", path);
      tryUpdateDir(res, true);
    } else {
      FFI.setByName("read_remote_dir", path);
    }
  }

  goToParentDirectory(bool isLocal) {
    final fd = isLocal ? _currentLocalDir : _currentRemoteDir;
    openDirectory(fd.parent, isLocal);
  }

  sendFiles(String path, String to, bool showHidden, bool isRemote) {
    _jobCount++;
    final msg = {
      "id": _jobCount.toString(),
      "path": path,
      "to": to,
      "show_hidden": showHidden.toString(),
      "is_remote": isRemote.toString() // isRemote 指path的位置而不是to的位置
    };
    FFI.setByName("send_files",jsonEncode(msg));
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
