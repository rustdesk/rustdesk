import 'dart:io';
import 'dart:math';

import 'package:desktop_drop/desktop_drop.dart';
import 'package:flutter/material.dart';
import 'package:flutter_breadcrumb/flutter_breadcrumb.dart';
import 'package:flutter_hbb/mobile/pages/file_manager_page.dart';
import 'package:flutter_hbb/models/file_model.dart';
import 'package:get/get.dart';
import 'package:provider/provider.dart';
import 'package:wakelock/wakelock.dart';

import '../../common.dart';
import '../../models/model.dart';
import '../../models/platform_model.dart';

enum LocationStatus { bread, textField }

class FileManagerPage extends StatefulWidget {
  const FileManagerPage({Key? key, required this.id}) : super(key: key);
  final String id;

  @override
  State<StatefulWidget> createState() => _FileManagerPageState();
}

class _FileManagerPageState extends State<FileManagerPage>
    with AutomaticKeepAliveClientMixin {
  final _localSelectedItems = SelectedItems();
  final _remoteSelectedItems = SelectedItems();

  final _locationStatusLocal = LocationStatus.bread.obs;
  final _locationStatusRemote = LocationStatus.bread.obs;
  final FocusNode _locationNodeLocal =
      FocusNode(debugLabel: "locationNodeLocal");
  final FocusNode _locationNodeRemote =
      FocusNode(debugLabel: "locationNodeRemote");
  final _searchTextLocal = "".obs;
  final _searchTextRemote = "".obs;
  final _breadCrumbScrollerLocal = ScrollController();
  final _breadCrumbScrollerRemote = ScrollController();

  final _dropMaskVisible = false.obs;

  ScrollController getBreadCrumbScrollController(bool isLocal) {
    return isLocal ? _breadCrumbScrollerLocal : _breadCrumbScrollerRemote;
  }

  late FFI _ffi;

  FileModel get model => _ffi.fileModel;

  SelectedItems getSelectedItem(bool isLocal) {
    return isLocal ? _localSelectedItems : _remoteSelectedItems;
  }

  @override
  void initState() {
    super.initState();
    _ffi = FFI();
    _ffi.connect(widget.id, isFileTransfer: true);
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _ffi.dialogManager
          .showLoading(translate('Connecting...'), onCancel: closeConnection);
    });
    Get.put(_ffi, tag: 'ft_${widget.id}');
    if (!Platform.isLinux) {
      Wakelock.enable();
    }
    debugPrint("File manager page init success with id ${widget.id}");
    // register location listener
    _locationNodeLocal.addListener(onLocalLocationFocusChanged);
    _locationNodeRemote.addListener(onRemoteLocationFocusChanged);
  }

  @override
  void dispose() {
    model.onClose();
    _ffi.close();
    _ffi.dialogManager.dismissAll();
    if (!Platform.isLinux) {
      Wakelock.disable();
    }
    Get.delete<FFI>(tag: 'ft_${widget.id}');
    _locationNodeLocal.removeListener(onLocalLocationFocusChanged);
    _locationNodeRemote.removeListener(onRemoteLocationFocusChanged);
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    super.build(context);
    return Overlay(initialEntries: [
      OverlayEntry(builder: (context) {
        _ffi.dialogManager.setOverlayState(Overlay.of(context));
        return ChangeNotifierProvider.value(
            value: _ffi.fileModel,
            child: Consumer<FileModel>(builder: (_context, _model, _child) {
              return WillPopScope(
                  onWillPop: () async {
                    if (model.selectMode) {
                      model.toggleSelectMode();
                    }
                    return false;
                  },
                  child: Scaffold(
                    backgroundColor: Theme.of(context).backgroundColor,
                    body: Row(
                      children: [
                        Flexible(flex: 3, child: body(isLocal: true)),
                        Flexible(flex: 3, child: body(isLocal: false)),
                        Flexible(flex: 2, child: statusList())
                      ],
                    ),
                  ));
            }));
      })
    ]);
  }

  Widget menu({bool isLocal = false}) {
    return PopupMenuButton<String>(
        icon: const Icon(Icons.more_vert),
        splashRadius: 20,
        itemBuilder: (context) {
          return [
            PopupMenuItem(
              child: Row(
                children: [
                  Icon(
                      model.getCurrentShowHidden(isLocal)
                          ? Icons.check_box_outlined
                          : Icons.check_box_outline_blank,
                      color: Colors.black),
                  SizedBox(width: 5),
                  Text(translate("Show Hidden Files"))
                ],
              ),
              value: "hidden",
            )
          ];
        },
        onSelected: (v) {
          if (v == "hidden") {
            model.toggleShowHidden(local: isLocal);
          }
        });
  }

  Widget body({bool isLocal = false}) {
    final fd = model.getCurrentDir(isLocal);
    final entries = fd.entries;
    final sortIndex = (SortBy style) {
      switch (style) {
        case SortBy.Name:
          return 1;
        case SortBy.Type:
          return 0;
        case SortBy.Modified:
          return 2;
        case SortBy.Size:
          return 3;
      }
    }(model.getSortStyle(isLocal));
    final sortAscending =
        isLocal ? model.localSortAscending : model.remoteSortAscending;
    return Container(
      decoration: BoxDecoration(border: Border.all(color: Colors.black26)),
      margin: const EdgeInsets.all(16.0),
      padding: const EdgeInsets.all(8.0),
      child: DropTarget(
        onDragDone: (detail) => handleDragDone(detail, isLocal),
        onDragEntered: (enter) {
          _dropMaskVisible.value = true;
        },
        onDragExited: (exit) {
          _dropMaskVisible.value = false;
        },
        child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          headTools(isLocal),
          Expanded(
              child: Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Expanded(
                child: SingleChildScrollView(
                  controller: ScrollController(),
                  child: ObxValue<RxString>(
                    (searchText) {
                      final filteredEntries = searchText.isEmpty
                          ? entries.where((element) {
                              if (searchText.isEmpty) {
                                return true;
                              } else {
                                return element.name.contains(searchText.value);
                              }
                            }).toList(growable: false)
                          : entries;
                      return DataTable(
                        key: ValueKey(isLocal ? 0 : 1),
                        showCheckboxColumn: true,
                        dataRowHeight: 25,
                        headingRowHeight: 30,
                        columnSpacing: 8,
                        showBottomBorder: true,
                        sortColumnIndex: sortIndex,
                        sortAscending: sortAscending,
                        columns: [
                          DataColumn(label: Text(translate(" "))), // icon
                          DataColumn(
                              label: Text(
                                translate("Name"),
                              ),
                              onSort: (columnIndex, ascending) {
                                model.changeSortStyle(SortBy.Name,
                                    isLocal: isLocal, ascending: ascending);
                              }),
                          DataColumn(
                              label: Text(
                                translate("Modified"),
                              ),
                              onSort: (columnIndex, ascending) {
                                model.changeSortStyle(SortBy.Modified,
                                    isLocal: isLocal, ascending: ascending);
                              }),
                          DataColumn(
                              label: Text(translate("Size")),
                              onSort: (columnIndex, ascending) {
                                model.changeSortStyle(SortBy.Size,
                                    isLocal: isLocal, ascending: ascending);
                              }),
                        ],
                        rows: filteredEntries.map((entry) {
                          final sizeStr = entry.isFile
                              ? readableFileSize(entry.size.toDouble())
                              : "";
                          return DataRow(
                              key: ValueKey(entry.name),
                              onSelectChanged: (s) {
                                if (s != null) {
                                  if (s) {
                                    getSelectedItem(isLocal)
                                        .add(isLocal, entry);
                                  } else {
                                    getSelectedItem(isLocal).remove(entry);
                                  }
                                  setState(() {});
                                }
                              },
                              selected:
                                  getSelectedItem(isLocal).contains(entry),
                              cells: [
                                DataCell(Icon(
                                    entry.isFile
                                        ? Icons.feed_outlined
                                        : Icons.folder,
                                    size: 25)),
                                DataCell(
                                    ConstrainedBox(
                                        constraints:
                                            BoxConstraints(maxWidth: 100),
                                        child: Tooltip(
                                          message: entry.name,
                                          child: Text(entry.name,
                                              overflow: TextOverflow.ellipsis),
                                        )), onTap: () {
                                  if (entry.isDirectory) {
                                    openDirectory(entry.path, isLocal: isLocal);
                                    if (isLocal) {
                                      _localSelectedItems.clear();
                                    } else {
                                      _remoteSelectedItems.clear();
                                    }
                                  } else {
                                    // Perform file-related tasks.
                                    final _selectedItems =
                                        getSelectedItem(isLocal);
                                    if (_selectedItems.contains(entry)) {
                                      _selectedItems.remove(entry);
                                    } else {
                                      _selectedItems.add(isLocal, entry);
                                    }
                                    setState(() {});
                                  }
                                }),
                                DataCell(Text(
                                  entry
                                          .lastModified()
                                          .toString()
                                          .replaceAll(".000", "") +
                                      "   ",
                                  style: TextStyle(
                                      fontSize: 12, color: MyTheme.darkGray),
                                )),
                                DataCell(Text(
                                  sizeStr,
                                  style: TextStyle(
                                      fontSize: 12, color: MyTheme.darkGray),
                                )),
                              ]);
                        }).toList(growable: false),
                      );
                    },
                    isLocal ? _searchTextLocal : _searchTextRemote,
                  ),
                ),
              )
            ],
          )),
        ]),
      ),
    );
  }

  /// transfer status list
  /// watch transfer status
  Widget statusList() {
    return PreferredSize(
        preferredSize: const Size(200, double.infinity),
        child: Container(
          margin: const EdgeInsets.only(top: 16.0, bottom: 16.0, right: 16.0),
          padding: const EdgeInsets.all(8.0),
          decoration: BoxDecoration(border: Border.all(color: Colors.grey)),
          child: Obx(
            () => ListView.builder(
              controller: ScrollController(),
              itemBuilder: (BuildContext context, int index) {
                final item = model.jobTable[index];
                return Column(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Row(
                      crossAxisAlignment: CrossAxisAlignment.center,
                      children: [
                        Transform.rotate(
                            angle: item.isRemote ? pi : 0,
                            child: const Icon(Icons.send)),
                        const SizedBox(
                          width: 16.0,
                        ),
                        Expanded(
                          child: Column(
                            mainAxisSize: MainAxisSize.min,
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                              Tooltip(
                                  message: item.jobName,
                                  child: Text(
                                    item.jobName,
                                    maxLines: 1,
                                    overflow: TextOverflow.ellipsis,
                                  )),
                              Wrap(
                                children: [
                                  Text(
                                      '${item.state.display()} ${max(0, item.fileNum)}/${item.fileCount} '),
                                  Text(
                                      '${translate("files")} ${readableFileSize(item.totalSize.toDouble())} '),
                                  Offstage(
                                      offstage:
                                          item.state != JobState.inProgress,
                                      child: Text(
                                          '${"${readableFileSize(item.speed)}/s"} ')),
                                  Offstage(
                                    offstage: item.totalSize <= 0,
                                    child: Text(
                                        '${(item.finishedSize.toDouble() * 100 / item.totalSize.toDouble()).toStringAsFixed(2)}%'),
                                  ),
                                ],
                              ),
                            ],
                          ),
                        ),
                        Row(
                          mainAxisAlignment: MainAxisAlignment.end,
                          children: [
                            Offstage(
                              offstage: item.state != JobState.paused,
                              child: IconButton(
                                  onPressed: () {
                                    model.resumeJob(item.id);
                                  },
                                  splashRadius: 20,
                                  icon: const Icon(Icons.restart_alt_rounded)),
                            ),
                            IconButton(
                              icon: const Icon(Icons.delete),
                              splashRadius: 20,
                              onPressed: () {
                                model.jobTable.removeAt(index);
                                model.cancelJob(item.id);
                              },
                            ),
                          ],
                        )
                      ],
                    ),
                    SizedBox(
                      height: 8.0,
                    ),
                    Divider(
                      height: 2.0,
                    )
                  ],
                );
              },
              itemCount: model.jobTable.length,
            ),
          ),
        ));
  }

  goBack({bool? isLocal}) {
    model.goToParentDirectory(isLocal: isLocal);
  }

  Widget headTools(bool isLocal) {
    final _locationStatus =
        isLocal ? _locationStatusLocal : _locationStatusRemote;
    final _locationFocus = isLocal ? _locationNodeLocal : _locationNodeRemote;
    final _searchTextObs = isLocal ? _searchTextLocal : _searchTextRemote;
    return Container(
        child: Column(
      children: [
        // symbols
        PreferredSize(
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.center,
              children: [
                Container(
                    width: 50,
                    height: 50,
                    decoration: BoxDecoration(color: Colors.blue),
                    padding: EdgeInsets.all(8.0),
                    child: FutureBuilder<String>(
                        future: bind.sessionGetPlatform(
                            id: _ffi.id, isRemote: !isLocal),
                        builder: (context, snapshot) {
                          if (snapshot.hasData && snapshot.data!.isNotEmpty) {
                            return getPlatformImage('${snapshot.data}');
                          } else {
                            return CircularProgressIndicator(
                              color: Colors.white,
                            );
                          }
                        })),
                Text(isLocal
                        ? translate("Local Computer")
                        : translate("Remote Computer"))
                    .marginOnly(left: 8.0)
              ],
            ),
            preferredSize: Size(double.infinity, 70)),
        // buttons
        Row(
          children: [
            Row(
              children: [
                IconButton(
                  onPressed: () {
                    model.goHome(isLocal: isLocal);
                  },
                  icon: const Icon(Icons.home_outlined),
                  splashRadius: 20,
                ),
                IconButton(
                  icon: const Icon(Icons.arrow_upward),
                  splashRadius: 20,
                  onPressed: () {
                    goBack(isLocal: isLocal);
                  },
                ),
                menu(isLocal: isLocal),
              ],
            ),
            Expanded(
                child: GestureDetector(
              onTap: () {
                _locationStatus.value =
                    _locationStatus.value == LocationStatus.bread
                        ? LocationStatus.textField
                        : LocationStatus.bread;
                Future.delayed(Duration.zero, () {
                  if (_locationStatus.value == LocationStatus.textField) {
                    _locationFocus.requestFocus();
                  }
                });
              },
              child: Container(
                  decoration:
                      BoxDecoration(border: Border.all(color: Colors.black12)),
                  child: Row(
                    children: [
                      Expanded(
                          child: Obx(() =>
                              _locationStatus.value == LocationStatus.bread
                                  ? buildBread(isLocal)
                                  : buildPathLocation(isLocal))),
                      DropdownButton<String>(
                          isDense: true,
                          underline: Offstage(),
                          items: [
                            // TODO: favourite
                            DropdownMenuItem(
                              child: Text('/'),
                              value: '/',
                            )
                          ],
                          onChanged: (path) {
                            if (path is String && path.isNotEmpty) {
                              openDirectory(path, isLocal: isLocal);
                            }
                          })
                    ],
                  )),
            )),
            PopupMenuButton(
              itemBuilder: (context) => [
                PopupMenuItem(
                    enabled: false,
                    child: ConstrainedBox(
                      constraints: BoxConstraints(minWidth: 200),
                      child: TextField(
                        controller:
                            TextEditingController(text: _searchTextObs.value),
                        autofocus: true,
                        decoration:
                            InputDecoration(prefixIcon: Icon(Icons.search)),
                        onChanged: (searchText) =>
                            onSearchText(searchText, isLocal),
                      ),
                    ))
              ],
              splashRadius: 20,
              child: const Icon(Icons.search),
            ),
            IconButton(
                onPressed: () {
                  model.refresh(isLocal: isLocal);
                },
                splashRadius: 20,
                icon: const Icon(Icons.refresh)),
          ],
        ),
        Row(
          textDirection: isLocal ? TextDirection.ltr : TextDirection.rtl,
          children: [
            Expanded(
              child: Row(
                mainAxisAlignment:
                    isLocal ? MainAxisAlignment.start : MainAxisAlignment.end,
                children: [
                  IconButton(
                      onPressed: () {
                        final name = TextEditingController();
                        _ffi.dialogManager.show((setState, close) {
                          submit() {
                            if (name.value.text.isNotEmpty) {
                              model.createDir(
                                  PathUtil.join(
                                      model.getCurrentDir(isLocal).path,
                                      name.value.text,
                                      model.getCurrentIsWindows(isLocal)),
                                  isLocal: isLocal);
                              close();
                            }
                          }

                          cancel() => close(false);
                          return CustomAlertDialog(
                            title: Text(translate("Create Folder")),
                            content: Column(
                              mainAxisSize: MainAxisSize.min,
                              children: [
                                TextFormField(
                                  decoration: InputDecoration(
                                    labelText: translate(
                                        "Please enter the folder name"),
                                  ),
                                  controller: name,
                                  focusNode: FocusNode()..requestFocus(),
                                ),
                              ],
                            ),
                            actions: [
                              TextButton(
                                  style: flatButtonStyle,
                                  onPressed: cancel,
                                  child: Text(translate("Cancel"))),
                              ElevatedButton(
                                  style: flatButtonStyle,
                                  onPressed: submit,
                                  child: Text(translate("OK")))
                            ],
                            onSubmit: submit,
                            onCancel: cancel,
                          );
                        });
                      },
                      splashRadius: 20,
                      icon: const Icon(Icons.create_new_folder_outlined)),
                  IconButton(
                      onPressed: () async {
                        final items = isLocal
                            ? _localSelectedItems
                            : _remoteSelectedItems;
                        await (model.removeAction(items, isLocal: isLocal));
                        items.clear();
                      },
                      splashRadius: 20,
                      icon: const Icon(Icons.delete_forever_outlined)),
                ],
              ),
            ),
            TextButton.icon(
                onPressed: () {
                  final items = getSelectedItem(isLocal);
                  model.sendFiles(items, isRemote: !isLocal);
                  items.clear();
                },
                icon: Transform.rotate(
                  angle: isLocal ? 0 : pi,
                  child: const Icon(
                    Icons.send,
                  ),
                ),
                label: Text(
                  isLocal ? translate('Send') : translate('Receive'),
                )),
          ],
        ).marginOnly(top: 8.0)
      ],
    ));
  }

  @override
  bool get wantKeepAlive => true;

  void onLocalLocationFocusChanged() {
    debugPrint("focus changed on local");
    if (_locationNodeLocal.hasFocus) {
      // ignore
    } else {
      // lost focus, change to bread
      _locationStatusLocal.value = LocationStatus.bread;
    }
  }

  void onRemoteLocationFocusChanged() {
    debugPrint("focus changed on remote");
    if (_locationNodeRemote.hasFocus) {
      // ignore
    } else {
      // lost focus, change to bread
      _locationStatusRemote.value = LocationStatus.bread;
    }
  }

  Widget buildBread(bool isLocal) {
    final items = getPathBreadCrumbItems(isLocal, (list) {
      var path = "";
      for (var item in list) {
        path = PathUtil.join(path, item, model.getCurrentIsWindows(isLocal));
      }
      openDirectory(path, isLocal: isLocal);
    });
    return items.isEmpty
        ? Offstage()
        : BreadCrumb(
            items: items,
            divider: Text("/").paddingSymmetric(horizontal: 4.0),
            overflow: ScrollableOverflow(
                controller: getBreadCrumbScrollController(isLocal)),
          );
  }

  List<BreadCrumbItem> getPathBreadCrumbItems(
      bool isLocal, void Function(List<String>) onPressed) {
    final path = model.getCurrentDir(isLocal).path;
    final list = PathUtil.split(path, model.getCurrentIsWindows(isLocal));
    final breadCrumbList = List<BreadCrumbItem>.empty(growable: true);
    breadCrumbList.addAll(list.asMap().entries.map((e) => BreadCrumbItem(
        content: TextButton(
            child: Text(e.value),
            style:
                ButtonStyle(minimumSize: MaterialStateProperty.all(Size(0, 0))),
            onPressed: () => onPressed(list.sublist(0, e.key + 1))))));
    return breadCrumbList;
  }

  breadCrumbScrollToEnd(bool isLocal) {
    Future.delayed(Duration(milliseconds: 200), () {
      final _breadCrumbScroller = getBreadCrumbScrollController(isLocal);
      _breadCrumbScroller.animateTo(
          _breadCrumbScroller.position.maxScrollExtent,
          duration: Duration(milliseconds: 200),
          curve: Curves.fastLinearToSlowEaseIn);
    });
  }

  Widget buildPathLocation(bool isLocal) {
    return TextField(
      focusNode: isLocal ? _locationNodeLocal : _locationNodeRemote,
      decoration: InputDecoration(
        border: InputBorder.none,
        isDense: true,
        prefix: Padding(padding: EdgeInsets.only(left: 4.0)),
      ),
      controller:
          TextEditingController(text: model.getCurrentDir(isLocal).path),
      onSubmitted: (path) {
        openDirectory(path, isLocal: isLocal);
      },
    );
  }

  onSearchText(String searchText, bool isLocal) {
    if (isLocal) {
      _searchTextLocal.value = searchText;
    } else {
      _searchTextRemote.value = searchText;
    }
  }

  openDirectory(String path, {bool isLocal = false}) {
    model.openDirectory(path, isLocal: isLocal).then((_) {
      breadCrumbScrollToEnd(isLocal);
    });
  }

  void handleDragDone(DropDoneDetails details, bool isLocal) {
    if (isLocal) {
      // ignore local
      return;
    }
    var items = SelectedItems();
    details.files.forEach((file) {
      final f = File(file.path);
      items.add(
          true,
          Entry()
            ..path = file.path
            ..name = file.name
            ..size =
                FileSystemEntity.isDirectorySync(f.path) ? 0 : f.lengthSync());
    });
    model.sendFiles(items, isRemote: false);
  }
}
