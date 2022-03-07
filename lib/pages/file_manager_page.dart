import 'dart:async';
import 'package:file_manager/file_manager.dart';
import 'package:flutter/material.dart';
import 'package:flutter_easyloading/flutter_easyloading.dart';
import 'package:flutter_hbb/models/file_model.dart';
import 'package:provider/provider.dart';

import '../common.dart';
import '../models/model.dart';
import '../widgets/dialog.dart';

class FileManagerPage extends StatefulWidget {
  FileManagerPage({Key? key, required this.id}) : super(key: key);
  final String id;

  @override
  State<StatefulWidget> createState() => _FileManagerPageState();
}

class _FileManagerPageState extends State<FileManagerPage> {
  final _localFileModel = FileManagerController();
  final _remoteFileModel = FFI.remoteFileModel;
  Timer? _interval;
  Timer? _timer;
  var _reconnects = 1;
  var _isLocal = false;

  @override
  void initState() {
    super.initState();
    showLoading(translate('Connecting...'));
    FFI.connect(widget.id, isFileTransfer: true);
    _interval = Timer.periodic(Duration(milliseconds: 30),
        (timer) => FFI.ffiModel.update(widget.id, context, handleMsgBox));
  }

  @override
  void dispose() {
    _localFileModel.dispose();
    _remoteFileModel.dispose();
    _interval?.cancel();
    FFI.close();
    EasyLoading.dismiss();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return ChangeNotifierProvider.value(
        value: _remoteFileModel,
        child: Scaffold(
          appBar: AppBar(
            leading: Row(children: [
              IconButton(icon: Icon(Icons.arrow_back), onPressed: goBack),
              IconButton(icon: Icon(Icons.close), onPressed: clientClose),
            ]),
            leadingWidth: 200,
            centerTitle: true,
            title: Text(translate(_isLocal ? "Local" : "Remote")),
            actions: [
              IconButton(
                  icon: Icon(Icons.change_circle),
                  onPressed: () => setState(() {
                        _isLocal = !_isLocal;
                      })),
            ],
          ),
          body:
              Consumer<RemoteFileModel>(builder: (context, remoteModel, child) {
            return FileManager(
                controller: _localFileModel,
                builder: (context, localSnapshot) {
                  final snapshot = _isLocal
                      ? localSnapshot
                      : remoteModel.currentRemoteDir.entries;
                  return ListView.builder(
                    itemCount: snapshot.length,
                    itemBuilder: (context, index) {
                      return Card(
                        child: ListTile(
                          leading: FileManager.isFile(snapshot[index])
                              ? Icon(Icons.feed_outlined)
                              : Icon(Icons.folder),
                          title: Text(FileManager.basename(snapshot[index])),
                          onTap: () {
                            if (FileManager.isDirectory(snapshot[index])) {
                              _isLocal
                                  ? _localFileModel
                                      .openDirectory(snapshot[index])
                                  : readRemoteDir(
                                      snapshot[index].path); // open directory
                            } else {
                              // Perform file-related tasks.
                            }
                          },
                        ),
                      );
                    },
                  );
                });
          }),
        ));
  }

  goBack() {
    if (_isLocal) {
      _localFileModel.goToParentDirectory();
    } else {
      _remoteFileModel.goToParentDirectory();
    }
  }

  void readRemoteDir(String path) {
    FFI.setByName("read_remote_dir", path);
  }

  void handleMsgBox(Map<String, dynamic> evt, String id) {
    var type = evt['type'];
    var title = evt['title'];
    var text = evt['text'];
    if (type == 're-input-password') {
      wrongPasswordDialog(id);
    } else if (type == 'input-password') {
      enterPasswordDialog(id);
    } else {
      var hasRetry = evt['hasRetry'] == 'true';
      print(evt);
      showMsgBox(type, title, text, hasRetry);
    }
  }

  void showMsgBox(String type, String title, String text, bool hasRetry) {
    msgBox(type, title, text);
    if (hasRetry) {
      _timer?.cancel();
      _timer = Timer(Duration(seconds: _reconnects), () {
        FFI.reconnect();
        showLoading(translate('Connecting...'));
      });
      _reconnects *= 2;
    } else {
      _reconnects = 1;
    }
  }
}
