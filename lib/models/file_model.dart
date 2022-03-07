import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:flutter/material.dart';

import 'model.dart';


// enum FileType {
//   Dir = 0,
//   DirLink = 2,
//   DirDrive = 3,
//   File = 4,
//   FileLink = 5,
// }

class FileDirectory {
  // List<Entry> entries = [];
  List<FileSystemEntity> entries = [];
  int id = 0;
  String path = "";

  FileDirectory();

  FileDirectory.fromJson(Map<String, dynamic> json) {
    id = json['id'];
    path = json['path'];
    if (json['entries'] != null) {
      entries = <FileSystemEntity>[];
      json['entries'].forEach((v) {
        entries.add(new Entry.fromJson(v).toFileSystemEntity(path));
      });
    }
  }

  // Map<String, dynamic> toJson() {
  //   final Map<String, dynamic> data = new Map<String, dynamic>();
  //   data['entries'] = this.entries.map((v) => v.toJson()).toList();
  //   data['id'] = this.id;
  //   data['path'] = this.path;
  //   return data;
  // }

  clear(){
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

  FileSystemEntity toFileSystemEntity(String parentPath){
    // is dir
    if(entryType<=3){
      return RemoteDir("$parentPath/$name");
    }else {
      return RemoteFile("$parentPath/$name",modifiedTime,size);
    }
  }

  Map<String, dynamic> toJson() {
    final Map<String, dynamic> data = new Map<String, dynamic>();
    data['entry_type'] = this.entryType;
    data['modified_time'] = this.modifiedTime;
    data['name'] = this.name;
    data['size'] = this.size;
    return data;
  }
}


// TODO 使用工厂单例模式

class RemoteFileModel extends ChangeNotifier{

  FileDirectory _currentRemoteDir = FileDirectory();

  FileDirectory get currentRemoteDir => _currentRemoteDir;

  tryUpdateRemoteDir(String fd){
    debugPrint("tryUpdateRemoteDir:$fd");
    try{
      final fileDir = FileDirectory.fromJson(jsonDecode(fd));
      _currentRemoteDir = fileDir;
      debugPrint("_currentRemoteDir:${_currentRemoteDir.path}");
      notifyListeners();
    }catch(e){
      debugPrint("tryUpdateRemoteDir fail:$fd");
    }
  }

  goToParentDirectory(){
    var parentPath = "";
    if(_currentRemoteDir.path == ""){
      parentPath = "";
    }else{
      parentPath = Directory(_currentRemoteDir.path).parent.path;
    }
    FFI.setByName("read_remote_dir", parentPath);
  }
  @override
  void dispose() {
    _currentRemoteDir.clear();
    super.dispose();
  }
}


class RemoteDir extends FileSystemEntity implements Directory{

  // int entryType = 4;
  // int modifiedTime = 0;
  // String name = "";
  // int size = 0;


  String path;

  RemoteDir(this.path);


  @override
  // TODO: implement absolute
  Directory get absolute => throw UnimplementedError();

  @override
  Future<Directory> create({bool recursive = false}) {
    // TODO: implement create
    throw UnimplementedError();
  }

  @override
  void createSync({bool recursive = false}) {
    // TODO: implement createSync
  }

  @override
  Future<Directory> createTemp([String? prefix]) {
    // TODO: implement createTemp
    throw UnimplementedError();
  }

  @override
  Directory createTempSync([String? prefix]) {
    // TODO: implement createTempSync
    throw UnimplementedError();
  }

  @override
  Future<bool> exists() {
    // TODO: implement exists
    throw UnimplementedError();
  }

  @override
  bool existsSync() {
    // TODO: implement existsSync
    throw UnimplementedError();
  }

  @override
  Stream<FileSystemEntity> list({bool recursive = false, bool followLinks = true}) {
    // TODO: implement list
    throw UnimplementedError();
  }

  @override
  List<FileSystemEntity> listSync({bool recursive = false, bool followLinks = true}) {
    // TODO: implement listSync
    throw UnimplementedError();
  }

  @override
  Future<Directory> rename(String newPath) {
    // TODO: implement rename
    throw UnimplementedError();
  }

  @override
  Directory renameSync(String newPath) {
    // TODO: implement renameSync
    throw UnimplementedError();
  }

}

class RemoteFile extends FileSystemEntity implements File {

  // int entryType = 4;
  // int modifiedTime = 0;
  // String name = "";
  // int size = 0;

  RemoteFile(this.path,this.modifiedTime,this.size);

  var path;
  var modifiedTime;
  var size;


  @override
  DateTime lastModifiedSync() {
    return DateTime.fromMillisecondsSinceEpoch(modifiedTime * 1000);
  }

  @override
  int lengthSync() {
    return size;
  }

  // ***************************

  @override
  Future<int> length() {
    // TODO: implement length
    throw UnimplementedError();
  }

  @override
  Future<DateTime> lastModified() {
    // TODO: implement lastModified
    throw UnimplementedError();
  }

  @override
  Future<File> copy(String newPath) {
    // TODO: implement copy
    throw UnimplementedError();
  }

  @override
  File copySync(String newPath) {
    // TODO: implement copySync
    throw UnimplementedError();
  }

  @override
  Future<File> create({bool recursive = false}) {
    // TODO: implement create
    throw UnimplementedError();
  }

  @override
  void createSync({bool recursive = false}) {
    // TODO: implement createSync
  }

  @override
  Future<bool> exists() {
    // TODO: implement exists
    throw UnimplementedError();
  }

  @override
  bool existsSync() {
    // TODO: implement existsSync
    throw UnimplementedError();
  }

  @override
  Future<DateTime> lastAccessed() {
    // TODO: implement lastAccessed
    throw UnimplementedError();
  }

  @override
  DateTime lastAccessedSync() {
    // TODO: implement lastAccessedSync
    throw UnimplementedError();
  }

  @override
  Future<RandomAccessFile> open({FileMode mode = FileMode.read}) {
    // TODO: implement open
    throw UnimplementedError();
  }

  @override
  Stream<List<int>> openRead([int? start, int? end]) {
    // TODO: implement openRead
    throw UnimplementedError();
  }

  @override
  RandomAccessFile openSync({FileMode mode = FileMode.read}) {
    // TODO: implement openSync
    throw UnimplementedError();
  }

  @override
  IOSink openWrite({FileMode mode = FileMode.write, Encoding encoding = utf8}) {
    // TODO: implement openWrite
    throw UnimplementedError();
  }

  @override
  Future<Uint8List> readAsBytes() {
    // TODO: implement readAsBytes
    throw UnimplementedError();
  }

  @override
  Uint8List readAsBytesSync() {
    // TODO: implement readAsBytesSync
    throw UnimplementedError();
  }

  @override
  Future<List<String>> readAsLines({Encoding encoding = utf8}) {
    // TODO: implement readAsLines
    throw UnimplementedError();
  }

  @override
  List<String> readAsLinesSync({Encoding encoding = utf8}) {
    // TODO: implement readAsLinesSync
    throw UnimplementedError();
  }

  @override
  Future<String> readAsString({Encoding encoding = utf8}) {
    // TODO: implement readAsString
    throw UnimplementedError();
  }

  @override
  String readAsStringSync({Encoding encoding = utf8}) {
    // TODO: implement readAsStringSync
    throw UnimplementedError();
  }

  @override
  Future<File> rename(String newPath) {
    // TODO: implement rename
    throw UnimplementedError();
  }

  @override
  File renameSync(String newPath) {
    // TODO: implement renameSync
    throw UnimplementedError();
  }

  @override
  Future setLastAccessed(DateTime time) {
    // TODO: implement setLastAccessed
    throw UnimplementedError();
  }

  @override
  void setLastAccessedSync(DateTime time) {
    // TODO: implement setLastAccessedSync
  }

  @override
  Future setLastModified(DateTime time) {
    // TODO: implement setLastModified
    throw UnimplementedError();
  }

  @override
  void setLastModifiedSync(DateTime time) {
    // TODO: implement setLastModifiedSync
  }

  @override
  Future<File> writeAsBytes(List<int> bytes, {FileMode mode = FileMode.write, bool flush = false}) {
    // TODO: implement writeAsBytes
    throw UnimplementedError();
  }

  @override
  void writeAsBytesSync(List<int> bytes, {FileMode mode = FileMode.write, bool flush = false}) {
    // TODO: implement writeAsBytesSync
  }

  @override
  Future<File> writeAsString(String contents, {FileMode mode = FileMode.write, Encoding encoding = utf8, bool flush = false}) {
    // TODO: implement writeAsString
    throw UnimplementedError();
  }

  @override
  void writeAsStringSync(String contents, {FileMode mode = FileMode.write, Encoding encoding = utf8, bool flush = false}) {
    // TODO: implement writeAsStringSync
  }

  @override
  // TODO: implement absolute
  File get absolute => throw UnimplementedError();
  
}