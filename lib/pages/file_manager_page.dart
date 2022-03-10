import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter_easyloading/flutter_easyloading.dart';
import 'package:flutter_hbb/models/file_model.dart';
import 'package:provider/provider.dart';
import 'package:flutter_breadcrumb/flutter_breadcrumb.dart';
import 'package:path/path.dart' as Path;

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
  final model = FFI.fileModel;
  final _selectedItems = SelectedItems();
  Timer? _interval;
  Timer? _timer;
  var _reconnects = 1;
  final _breadCrumbScroller = ScrollController();

  @override
  void initState() {
    super.initState();
    showLoading(translate('Connecting...'));
    FFI.connect(widget.id, isFileTransfer: true);

    final res = FFI.getByName("read_dir", FFI.getByName("get_home_dir"));
    debugPrint("read_dir local :$res");
    model.tryUpdateDir(res, true);

    _interval = Timer.periodic(Duration(milliseconds: 30),
        (timer) => FFI.ffiModel.update(widget.id, context, handleMsgBox));
  }

  @override
  void dispose() {
    model.clear();
    _interval?.cancel();
    FFI.close();
    EasyLoading.dismiss();
    super.dispose();
  }

  @override
  Widget build(BuildContext context)  => Consumer<FileModel>(builder: (_context, _model, _child) {
    return  WillPopScope(
        onWillPop: () async {
          if (model.selectMode) {
            model.toggleSelectMode();
          } else {
            goBack();
          }
          return false;
        },
        child: Scaffold(
          backgroundColor: MyTheme.grayBg,
          appBar: AppBar(
            leading: Row(children: [
              IconButton(icon: Icon(Icons.arrow_back), onPressed: goBack),
              IconButton(icon: Icon(Icons.close), onPressed: clientClose),
            ]),
            leadingWidth: 200,
            centerTitle: true,
            title: Text(translate(model.isLocal ? "Local" : "Remote")),
            actions: [
              IconButton(
                icon: Icon(Icons.change_circle),
                onPressed: ()=> model.togglePage(),
              )
            ],
          ),
          body: body(),
          bottomSheet: bottomSheet(),
        ));
  });

  bool needShowCheckBox(){
    if(!model.selectMode){
      return false;
    }
    return !_selectedItems.isOtherPage(model.isLocal);
  }

  Widget body() {
        final isLocal = model.isLocal;
        final fd = model.currentDir;
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
              final path = Path.join(fd.path, entries[index].name);
              var selected = false;
              if (model.selectMode) {
                selected = _selectedItems.contains(path);
              }
              return Card(
                child: ListTile(
                  leading: Icon(entries[index].isFile?Icons.feed_outlined:Icons.folder,
                        size: 40),

                  title: Text(entries[index].name),
                  selected: selected,
                  // subtitle:  Text(entries[index].lastModified().toString()),
                  trailing: needShowCheckBox()
                      ? Checkbox(
                          value: selected,
                          onChanged: (v) {
                            if (v == null) return;
                            if (v && !selected) {
                                _selectedItems.add(isLocal,path);
                            } else if (!v && selected) {
                                _selectedItems.remove(path);
                            }
                            setState(() {});
                          })
                      : null,
                  onTap: () {
                    if (model.selectMode && !_selectedItems.isOtherPage(isLocal)) {
                      if (selected) {
                        _selectedItems.remove(path);
                      } else {
                        _selectedItems.add(isLocal,path);
                      }
                      setState(() {});
                      return;
                    }
                    if (entries[index].isDirectory) {
                      model.openDirectory(path);
                      breadCrumbScrollToEnd();
                    } else {
                      // Perform file-related tasks.
                    }
                  },
                  onLongPress: () {
                    _selectedItems.clear();
                    model.toggleSelectMode();
                    if (model.selectMode) {
                      _selectedItems.add(isLocal,path);
                    }
                    setState(() {});
                  },
                ),
              );
            },
          ))
        ]);
      }

  goBack() {
    model.goToParentDirectory();
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

  breadCrumbScrollToEnd() {
    Future.delayed(Duration(milliseconds: 200), () {
      _breadCrumbScroller.animateTo(
          _breadCrumbScroller.position.maxScrollExtent,
          duration: Duration(milliseconds: 200),
          curve: Curves.fastLinearToSlowEaseIn);
    });
  }

  Widget headTools() => Container(
          child: Row(
        children: [
          Expanded(
              child: BreadCrumb(
            items: getPathBreadCrumbItems(() => debugPrint("pressed home"),
                (e) => debugPrint("pressed url:$e")),
            divider: Icon(Icons.chevron_right),
            overflow: ScrollableOverflow(controller: _breadCrumbScroller),
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
                  onSelected: model.changeSortStyle),
              PopupMenuButton<String>(
                  icon: Icon(Icons.more_vert),
                  itemBuilder: (context) {
                    return [
                      PopupMenuItem(
                        child: Row(
                          children: [Icon(Icons.refresh), Text("刷新")],
                        ),
                        value: "refresh",
                      ),
                      PopupMenuItem(
                        child: Row(
                          children: [Icon(Icons.check), Text("多选")],
                        ),
                        value: "select",
                      )
                    ];
                  },
                  onSelected: (v) {
                    if (v == "refresh") {
                      model.refresh();
                    } else if (v == "select") {
                      _selectedItems.clear();
                      model.toggleSelectMode();
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

  /// 有几种状态
  /// 选择模式 localPage
  /// 准备复制模式 otherPage
  /// 正在复制模式 动态数字和显示速度
  /// 粘贴完成模式
  BottomSheet? bottomSheet() {
    if (!model.selectMode) return null;
    return BottomSheet(
        backgroundColor: MyTheme.grayBg,
        enableDrag: false,
        onClosing: () {
          debugPrint("BottomSheet close");
        },
        builder: (context) {
          final isOtherPage = _selectedItems.isOtherPage(model.isLocal);
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
                  // 做一个bottomSheet类框架 不同状态下显示不同的内容
                  Row(
                    children: [
                      CircularProgressIndicator(),
                      isOtherPage?Icon(Icons.input):Icon(Icons.check),
                      SizedBox(width: 16),
                      Column(
                        mainAxisAlignment: MainAxisAlignment.center,
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(isOtherPage?'粘贴到这里?':'已选择',style: TextStyle(fontSize: 18)),
                          Text("${_selectedItems.length} 个文件 [${model.isLocal?'本地':'远程'}]",style: TextStyle(fontSize: 14,color: MyTheme.grayBg))
                        ],
                      )
                    ],
                  ),
                  Row(
                    children: [
                      (_selectedItems.length>0 && isOtherPage)? IconButton(
                        icon: Icon(Icons.paste),
                        onPressed:() {
                          debugPrint("paste");
                          // TODO　


                          model.sendFiles(
                              _selectedItems.items.first,
                              model.currentRemoteDir.path +
                                  '/' +
                                  _selectedItems.items.first.split('/').last,
                              false,
                              false);

                          // unused set callback
                          // _fileModel.set
                        },
                      ):IconButton(
                        icon: Icon(Icons.delete_forever),
                        onPressed: () {},
                      ),
                      IconButton(
                        icon: Icon(Icons.cancel_outlined),
                        onPressed: () {
                          model.toggleSelectMode();
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
    final path = model.currentDir.path;
    final list = path.trim().split('/'); // TODO use Path
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


class SelectedItems {
  bool? _isLocal;
  final List<String> _items = [];

  List<String> get items => _items;

  int get length => _items.length;

  // bool get isEmpty => _items.length == 0;

  add(bool isLocal, String path) {
    if (_isLocal == null) {
      _isLocal = isLocal;
    }
    if (_isLocal != null && _isLocal != isLocal) {
      return;
    }
    if (!_items.contains(path)) {
      _items.add(path);
    }
  }

  bool contains(String path) {
    return _items.contains(path);
  }

  remove(String path) {
    _items.remove(path);
    if (_items.length == 0) {
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
}
