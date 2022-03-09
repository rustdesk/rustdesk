import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter_easyloading/flutter_easyloading.dart';
import 'package:flutter_hbb/models/file_model.dart';
import 'package:provider/provider.dart';
import 'package:flutter_breadcrumb/flutter_breadcrumb.dart';
import 'package:path/path.dart' as p;

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
  final _fileModel = FFI.fileModel;
  Timer? _interval;
  Timer? _timer;
  var _reconnects = 1;

  var _isLocal = false;
  var _selectMode = false;
  final List<String> _selectedItems = []; // 换成entry对象数组


  @override
  void initState() {
    super.initState();
    showLoading(translate('Connecting...'));
    FFI.connect(widget.id, isFileTransfer: true);
    Future.delayed(Duration(seconds: 1), () {
      final res = FFI.getByName("read_dir", FFI.getByName("get_home_dir"));
      debugPrint("read_dir local :$res");
      _fileModel.tryUpdateDir(res, true);
    });
    _interval = Timer.periodic(Duration(milliseconds: 30),
        (timer) => FFI.ffiModel.update(widget.id, context, handleMsgBox));
  }

  @override
  void dispose() {
    _fileModel.clear();
    _interval?.cancel();
    FFI.close();
    EasyLoading.dismiss();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return ChangeNotifierProvider.value(
        value: _fileModel,
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
                }),
              )
            ],
          ),
          body: body(),
          bottomSheet: bottomSheet(),
        ));
  }

  Widget body() => Consumer<FileModel>(builder: (context, fileModel, _child) {
        final fd =
            _isLocal ? fileModel.currentLocalDir : fileModel.currentRemoteDir;
        final entries = fd.entries;
        return Column(children: [
          headTools(),
          Expanded(
              child: ListView.builder(
            itemCount: entries.length + 1,
            itemBuilder: (context, index) {
              if (index >= entries.length) {
                // 添加尾部信息 文件统计信息等
                // 添加快速返回上部
                // 使用 bottomSheet 提示以选择的文件数量 点击后展开查看更多
                return listTail();
              }
              final path = p.join(fd.path,entries[index].name);
              var selected = false;
              if (_selectMode) {
                selected = _selectedItems.any((e) => e == path);
              }
              return Card(
                child: ListTile(
                  leading: entries[index].isFile
                      ? Icon(Icons.feed_outlined)
                      : Icon(Icons.folder),
                  title: Text(entries[index].name),
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
                    if (entries[index].isDirectory) {
                      _fileModel.openDirectory(path,_isLocal);
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
      });

  goBack() {
    _fileModel.goToParentDirectory(_isLocal);
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
                    return SortBy.values
                        .map((e) => PopupMenuItem(
                              child:
                                  Text(translate(e.toString().split(".").last)),
                              value: e,
                            ))
                        .toList();
                  },
                  onSelected: _fileModel.changeSortStyle),
              PopupMenuButton<String>(
                  icon: Icon(Icons.more_vert),
                  itemBuilder: (context) {
                    return [
                      PopupMenuItem(
                        child: Row(
                          children: [
                            Icon(Icons.refresh),
                            Text("刷新")
                          ],
                        ),
                        value: "refresh",
                      )
                    ];
                  },
                  onSelected: (v){
                    if(v == "refresh"){
                      _fileModel.refresh(_isLocal);
                    }
                  }),
            ],
          )
        ],
      ));

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
                        onPressed: () {
                          debugPrint("paste");
                          _fileModel.sendFiles(_selectedItems.first, _fileModel.currentRemoteDir.path+'/'+_selectedItems.first.split('/').last, false, false);
                        },
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
        ? _fileModel.currentLocalDir.path
        : _fileModel.currentRemoteDir.path;
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

}
