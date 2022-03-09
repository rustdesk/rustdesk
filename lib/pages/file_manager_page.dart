import 'dart:async';
import 'dart:io';
import 'package:file_manager/file_manager.dart';
import 'package:flutter/material.dart';
import 'package:flutter_easyloading/flutter_easyloading.dart';
import 'package:flutter_hbb/models/file_model.dart';
import 'package:provider/provider.dart';
import 'package:flutter_breadcrumb/flutter_breadcrumb.dart';

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
  var _selectMode = false;
  final List<String> _selectedItems = [];

  // final _breadCrumbScrollController = ScrollController();

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
    _remoteFileModel.dispose();
    _localFileModel.dispose();
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
          backgroundColor: MyTheme.grayBg,
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
          body: body(),
          bottomSheet: bottomSheet(),
        ));
  }

  Widget body() => Consumer<RemoteFileModel>(
      builder: (context, remoteModel, _child) => FileManager(
          controller: _localFileModel,
          emptyFolder: emptyPage(),
          builder: (context, localSnapshot) {
            final snapshot =
                _isLocal ? localSnapshot : remoteModel.currentRemoteDir.entries;
            return Column(children: [
              headTools(),
              Expanded(
                  child: ListView.builder(
                itemCount: snapshot.length + 1,
                itemBuilder: (context, index) {
                  if (index >= snapshot.length) {
                    // 添加尾部信息 文件统计信息等
                    // 添加快速返回上部
                    // 使用 bottomSheet 提示以选择的文件数量 点击后展开查看更多
                    return listTail();
                  }

                  var isFile = false;
                  if (_isLocal){
                    isFile = FileManager.isFile(snapshot[index]);
                  }else {
                    isFile = (snapshot[index] as RemoteFileSystemEntity).isFile();
                  }

                  final path = snapshot[index].path;
                  var selected = false;
                  if (_selectMode) {
                    selected = _selectedItems.any((e) => e == path);
                  }
                  return Card(
                    child: ListTile(
                      leading: isFile
                            ? Icon(Icons.feed_outlined)
                            : Icon(Icons.folder),
                      title: Text(FileManager.basename(snapshot[index])),
                      trailing: _selectMode
                          ? Checkbox(
                              value: selected,
                              onChanged: (v) {
                                if (v == null) return;
                                if (v && !selected) {
                                  setState(() {
                                    _selectedItems.add(path);
                                  });
                                } else if (!v && selected) {
                                  setState(() {
                                    _selectedItems.remove(path);
                                  });
                                }
                              })
                          : null,
                      onTap: () {
                        if (!isFile) {
                          if (_isLocal) {
                            _localFileModel.openDirectory(snapshot[index]);
                          } else {
                            readRemoteDir(path);
                          }
                        } else {
                          // Perform file-related tasks.
                        }
                      },
                      onLongPress: () {
                        setState(() {
                          _selectedItems.clear();
                          _selectMode = !_selectMode;
                        });
                      },
                    ),
                  );
                },
              ))
            ]);
          }));

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

  Widget headTools() => Container(
          child: Row(
        children: [
          Expanded(
              child: BreadCrumb(
            items: getPathBreadCrumbItems(() => debugPrint("pressed home"),
                (e) => debugPrint("pressed url:$e")),
            divider: Icon(Icons.chevron_right),
            overflow: ScrollableOverflow(reverse: false), // TODO 计算容器宽度判断
          )),
          Row(
            children: [
              // IconButton(onPressed: () {}, icon: Icon(Icons.sort)),
              PopupMenuButton<SortBy>(
                icon: Icon(Icons.sort),
                  itemBuilder: (context) {
                    return SortBy.values.map((e) => PopupMenuItem(
                      child: Text(translate(e.toString().split(".").last)),
                      value: e,
                    )).toList();
                  },
                  onSelected: changeSortStyle),
              IconButton(onPressed: () {}, icon: Icon(Icons.more_vert)),
            ],
          )
        ],
      ));

  changeSortStyle(SortBy sort){
    if(_isLocal){
      _localFileModel.sortedBy = sort;
    }else{
      _remoteFileModel.changeSortStyle(sort);
    }
  }

  Widget emptyPage() {
    return Column(
      children: [
        headTools(),
        Expanded(child: Center(child: Text("Empty Directory")))
      ],
    );
  }

  Widget listTail() {
    return SizedBox(height: 100);
  }

  BottomSheet? bottomSheet() {
    if (!_selectMode) return null;
    return BottomSheet(
        backgroundColor: MyTheme.grayBg,
        enableDrag: false,
        onClosing: () {
          debugPrint("BottomSheet close");
        },
        builder: (context) {
          return Container(
            height: 65,
            alignment: Alignment.centerLeft,
            decoration: BoxDecoration(
                color: MyTheme.accent50,
                borderRadius: BorderRadius.vertical(top: Radius.circular(10))),
            child: Padding(
              padding: EdgeInsets.symmetric(horizontal: 15),
              child: Row(
                mainAxisAlignment: MainAxisAlignment.spaceBetween,
                children: [
                  Row(
                    children: [
                      Icon(Icons.check),
                      SizedBox(width: 5),
                      Text(
                        "已选择 ${_selectedItems.length}",
                        style: TextStyle(fontSize: 18),
                      ),
                    ],
                  ),
                  Row(
                    children: [
                      IconButton(
                        icon: Icon(Icons.paste),
                        onPressed: () {},
                      ),
                      IconButton(
                        icon: Icon(Icons.delete_forever),
                        onPressed: () {},
                      ),
                      IconButton(
                        icon: Icon(Icons.cancel_outlined),
                        onPressed: () {
                          setState(() {
                            _selectMode = false;
                          });
                        },
                      ),
                    ],
                  )
                ],
              ),
            ),
          );
        });
  }

  List<BreadCrumbItem> getPathBreadCrumbItems(
      void Function() onHome, void Function(String) onPressed) {
    final path = _isLocal
        ? _localFileModel.getCurrentPath
        : _remoteFileModel.currentRemoteDir.path;
    final list = path.trim().split('/');
    list.remove("");
    final breadCrumbList = [
      BreadCrumbItem(
          content: IconButton(
        icon: Icon(Icons.home_filled),
        onPressed: onHome,
      ))
    ];
    breadCrumbList.addAll(list.map((e) => BreadCrumbItem(
        content: TextButton(
            child: Text(e),
            style:
                ButtonStyle(minimumSize: MaterialStateProperty.all(Size(0, 0))),
            onPressed: () => onPressed(e)))));
    return breadCrumbList;
  }

// // NOT GOOD
// void breadCrumbToLast() {
//   try {
//     _breadCrumbScrollController.animateTo(
//       _breadCrumbScrollController.position.maxScrollExtent,
//       curve: Curves.easeOut,
//       duration: const Duration(milliseconds: 300),
//     );
//   } catch (e) {}
// }
}
