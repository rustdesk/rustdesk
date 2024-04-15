import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_breadcrumb/flutter_breadcrumb.dart';
import 'package:flutter_hbb/models/file_model.dart';
import 'package:get/get.dart';
import 'package:toggle_switch/toggle_switch.dart';
import 'package:wakelock_plus/wakelock_plus.dart';

import '../../common.dart';
import '../../common/widgets/dialog.dart';

class FileManagerPage extends StatefulWidget {
  FileManagerPage(
      {Key? key, required this.id, this.password, this.isSharedPassword})
      : super(key: key);
  final String id;
  final String? password;
  final bool? isSharedPassword;

  @override
  State<StatefulWidget> createState() => _FileManagerPageState();
}

enum SelectMode { local, remote, none }

extension SelectModeEq on SelectMode {
  bool eq(bool? currentIsLocal) {
    if (currentIsLocal == null) {
      return false;
    }
    if (currentIsLocal) {
      return this == SelectMode.local;
    } else {
      return this == SelectMode.remote;
    }
  }
}

extension SelectModeExt on Rx<SelectMode> {
  void toggle(bool currentIsLocal) {
    switch (value) {
      case SelectMode.local:
        value = SelectMode.none;
        break;
      case SelectMode.remote:
        value = SelectMode.none;
        break;
      case SelectMode.none:
        if (currentIsLocal) {
          value = SelectMode.local;
        } else {
          value = SelectMode.remote;
        }
        break;
    }
  }
}

class _FileManagerPageState extends State<FileManagerPage> {
  final model = gFFI.fileModel;
  final selectMode = SelectMode.none.obs;

  var showLocal = true;

  FileController get currentFileController =>
      showLocal ? model.localController : model.remoteController;
  FileDirectory get currentDir => currentFileController.directory.value;
  DirectoryOptions get currentOptions => currentFileController.options.value;

  @override
  void initState() {
    super.initState();
    gFFI.start(widget.id,
        isFileTransfer: true,
        password: widget.password,
        isSharedPassword: widget.isSharedPassword);
    WidgetsBinding.instance.addPostFrameCallback((_) {
      gFFI.dialogManager
          .showLoading(translate('Connecting...'), onCancel: closeConnection);
    });
    gFFI.ffiModel.updateEventListener(gFFI.sessionId, widget.id);
    WakelockPlus.enable();
  }

  @override
  void dispose() {
    model.close().whenComplete(() {
      gFFI.close();
      gFFI.dialogManager.dismissAll();
      WakelockPlus.disable();
    });
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => WillPopScope(
      onWillPop: () async {
        if (selectMode.value != SelectMode.none) {
          selectMode.value = SelectMode.none;
          setState(() {});
        } else {
          currentFileController.goBack();
        }
        return false;
      },
      child: Scaffold(
        // backgroundColor: MyTheme.grayBg,
        appBar: AppBar(
          leading: Row(children: [
            IconButton(
                icon: Icon(Icons.close),
                onPressed: () =>
                    clientClose(gFFI.sessionId, gFFI.dialogManager)),
          ]),
          centerTitle: true,
          title: ToggleSwitch(
            initialLabelIndex: showLocal ? 0 : 1,
            activeBgColor: [MyTheme.idColor],
            inactiveBgColor: Theme.of(context).brightness == Brightness.light
                ? MyTheme.grayBg
                : null,
            inactiveFgColor: Theme.of(context).brightness == Brightness.light
                ? Colors.black54
                : null,
            totalSwitches: 2,
            minWidth: 100,
            fontSize: 15,
            iconSize: 18,
            labels: [translate("Local"), translate("Remote")],
            icons: [Icons.phone_android_sharp, Icons.screen_share],
            onToggle: (index) {
              final current = showLocal ? 0 : 1;
              if (index != current) {
                setState(() => showLocal = !showLocal);
              }
            },
          ),
          actions: [
            PopupMenuButton<String>(
                tooltip: "",
                icon: Icon(Icons.more_vert),
                itemBuilder: (context) {
                  return [
                    PopupMenuItem(
                      child: Row(
                        children: [
                          Icon(Icons.refresh,
                              color: Theme.of(context).iconTheme.color),
                          SizedBox(width: 5),
                          Text(translate("Refresh File"))
                        ],
                      ),
                      value: "refresh",
                    ),
                    PopupMenuItem(
                      enabled: currentDir.path != "/",
                      child: Row(
                        children: [
                          Icon(Icons.check,
                              color: Theme.of(context).iconTheme.color),
                          SizedBox(width: 5),
                          Text(translate("Multi Select"))
                        ],
                      ),
                      value: "select",
                    ),
                    PopupMenuItem(
                      enabled: currentDir.path != "/",
                      child: Row(
                        children: [
                          Icon(Icons.folder_outlined,
                              color: Theme.of(context).iconTheme.color),
                          SizedBox(width: 5),
                          Text(translate("Create Folder"))
                        ],
                      ),
                      value: "folder",
                    ),
                    PopupMenuItem(
                      enabled: currentDir.path != "/",
                      child: Row(
                        children: [
                          Icon(
                              currentOptions.showHidden
                                  ? Icons.check_box_outlined
                                  : Icons.check_box_outline_blank,
                              color: Theme.of(context).iconTheme.color),
                          SizedBox(width: 5),
                          Text(translate("Show Hidden Files"))
                        ],
                      ),
                      value: "hidden",
                    )
                  ];
                },
                onSelected: (v) {
                  if (v == "refresh") {
                    currentFileController.refresh();
                  } else if (v == "select") {
                    model.localController.selectedItems.clear();
                    model.remoteController.selectedItems.clear();
                    selectMode.toggle(showLocal);
                    setState(() {});
                  } else if (v == "folder") {
                    final name = TextEditingController();
                    gFFI.dialogManager
                        .show((setState, close, context) => CustomAlertDialog(
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
                                    ),
                                  ],
                                ),
                                actions: [
                                  dialogButton("Cancel",
                                      onPressed: () => close(false),
                                      isOutline: true),
                                  dialogButton("OK", onPressed: () {
                                    if (name.value.text.isNotEmpty) {
                                      currentFileController.createDir(
                                          PathUtil.join(
                                              currentDir.path,
                                              name.value.text,
                                              currentOptions.isWindows));
                                      close();
                                    }
                                  })
                                ]));
                  } else if (v == "hidden") {
                    currentFileController.toggleShowHidden();
                  }
                }),
          ],
        ),
        body: showLocal
            ? FileManagerView(
                controller: model.localController,
                selectMode: selectMode,
              )
            : FileManagerView(
                controller: model.remoteController,
                selectMode: selectMode,
              ),
        bottomSheet: bottomSheet(),
      ));

  Widget? bottomSheet() {
    return Obx(() {
      final selectedItems = getActiveSelectedItems();
      final jobTable = model.jobController.jobTable;

      final localLabel = selectedItems?.isLocal == null
          ? ""
          : " [${selectedItems!.isLocal ? translate("Local") : translate("Remote")}]";
      if (!(selectMode.value == SelectMode.none)) {
        final selectedItemsLen =
            "${selectedItems?.items.length ?? 0} ${translate("items")}";
        if (selectedItems == null ||
            selectedItems.items.isEmpty ||
            selectMode.value.eq(showLocal)) {
          return BottomSheetBody(
              leading: Icon(Icons.check),
              title: translate("Selected"),
              text: selectedItemsLen + localLabel,
              onCanceled: () {
                selectedItems?.items.clear();
                selectMode.value = SelectMode.none;
                setState(() {});
              },
              actions: [
                IconButton(
                  icon: Icon(Icons.compare_arrows),
                  onPressed: () => setState(() => showLocal = !showLocal),
                ),
                IconButton(
                  icon: Icon(Icons.delete_forever),
                  onPressed: selectedItems != null
                      ? () async {
                          if (selectedItems.items.isNotEmpty) {
                            await currentFileController
                                .removeAction(selectedItems);
                            selectedItems.items.clear();
                            selectMode.value = SelectMode.none;
                          }
                        }
                      : null,
                )
              ]);
        } else {
          return BottomSheetBody(
              leading: Icon(Icons.input),
              title: translate("Paste here?"),
              text: selectedItemsLen + localLabel,
              onCanceled: () {
                selectedItems.items.clear();
                selectMode.value = SelectMode.none;
                setState(() {});
              },
              actions: [
                IconButton(
                  icon: Icon(Icons.compare_arrows),
                  onPressed: () => setState(() => showLocal = !showLocal),
                ),
                IconButton(
                  icon: Icon(Icons.paste),
                  onPressed: () {
                    selectMode.value = SelectMode.none;
                    final otherSide = showLocal
                        ? model.remoteController
                        : model.localController;
                    final thisSideData =
                        DirectoryData(currentDir, currentOptions);
                    otherSide.sendFiles(selectedItems, thisSideData);
                    selectedItems.items.clear();
                    selectMode.value = SelectMode.none;
                  },
                )
              ]);
        }
      }

      if (jobTable.isEmpty) {
        return Offstage();
      }

      switch (jobTable.last.state) {
        case JobState.inProgress:
          return BottomSheetBody(
            leading: CircularProgressIndicator(),
            title: translate("Waiting"),
            text:
                "${translate("Speed")}:  ${readableFileSize(jobTable.last.speed)}/s",
            onCanceled: () {
              model.jobController.cancelJob(jobTable.last.id);
              jobTable.clear();
            },
          );
        case JobState.done:
          return BottomSheetBody(
            leading: Icon(Icons.check),
            title: "${translate("Successful")}!",
            text: jobTable.last.display(),
            onCanceled: () => jobTable.clear(),
          );
        case JobState.error:
          return BottomSheetBody(
            leading: Icon(Icons.error),
            title: "${translate("Error")}!",
            text: "",
            onCanceled: () => jobTable.clear(),
          );
        case JobState.none:
          break;
        case JobState.paused:
          // TODO: Handle this case.
          break;
      }
      return Offstage();
    });
  }

  SelectedItems? getActiveSelectedItems() {
    final localSelectedItems = model.localController.selectedItems;
    final remoteSelectedItems = model.remoteController.selectedItems;

    if (localSelectedItems.items.isNotEmpty &&
        remoteSelectedItems.items.isNotEmpty) {
      // assert unreachable
      debugPrint("Wrong SelectedItems state, reset");
      localSelectedItems.clear();
      remoteSelectedItems.clear();
    }

    if (localSelectedItems.items.isEmpty && remoteSelectedItems.items.isEmpty) {
      return null;
    }

    if (localSelectedItems.items.length > remoteSelectedItems.items.length) {
      return localSelectedItems;
    } else {
      return remoteSelectedItems;
    }
  }
}

class FileManagerView extends StatefulWidget {
  final FileController controller;
  final Rx<SelectMode> selectMode;

  FileManagerView({required this.controller, required this.selectMode});

  @override
  State<StatefulWidget> createState() => _FileManagerViewState();
}

class _FileManagerViewState extends State<FileManagerView> {
  final _listScrollController = ScrollController();
  final _breadCrumbScroller = ScrollController();

  bool get isLocal => widget.controller.isLocal;
  FileController get controller => widget.controller;
  SelectedItems get _selectedItems => widget.controller.selectedItems;

  @override
  void initState() {
    super.initState();
    controller.directory.listen((e) => breadCrumbScrollToEnd());
  }

  @override
  Widget build(BuildContext context) {
    return Column(children: [
      headTools(),
      Expanded(child: Obx(() {
        final entries = controller.directory.value.entries;
        return ListView.builder(
          controller: _listScrollController,
          itemCount: entries.length + 1,
          itemBuilder: (context, index) {
            if (index >= entries.length) {
              return listTail();
            }
            var selected = false;
            if (widget.selectMode.value != SelectMode.none) {
              selected = _selectedItems.items.contains(entries[index]);
            }

            final sizeStr = entries[index].isFile
                ? readableFileSize(entries[index].size.toDouble())
                : "";

            final showCheckBox = () {
              return widget.selectMode.value != SelectMode.none &&
                  widget.selectMode.value.eq(controller.selectedItems.isLocal);
            }();
            return Card(
              child: ListTile(
                leading: entries[index].isDrive
                    ? Padding(
                        padding: EdgeInsets.symmetric(vertical: 8),
                        child: Image(
                            image: iconHardDrive,
                            fit: BoxFit.scaleDown,
                            color: Theme.of(context)
                                .iconTheme
                                .color
                                ?.withOpacity(0.7)))
                    : Icon(
                        entries[index].isFile
                            ? Icons.feed_outlined
                            : Icons.folder,
                        size: 40),
                title: Text(entries[index].name),
                selected: selected,
                subtitle: entries[index].isDrive
                    ? null
                    : Text(
                        "${entries[index].lastModified().toString().replaceAll(".000", "")}   $sizeStr",
                        style: TextStyle(fontSize: 12, color: MyTheme.darkGray),
                      ),
                trailing: entries[index].isDrive
                    ? null
                    : showCheckBox
                        ? Checkbox(
                            value: selected,
                            onChanged: (v) {
                              if (v == null) return;
                              if (v && !selected) {
                                _selectedItems.add(entries[index]);
                              } else if (!v && selected) {
                                _selectedItems.remove(entries[index]);
                              }
                              setState(() {});
                            })
                        : PopupMenuButton<String>(
                            tooltip: "",
                            icon: Icon(Icons.more_vert),
                            itemBuilder: (context) {
                              return [
                                PopupMenuItem(
                                  child: Text(translate("Delete")),
                                  value: "delete",
                                ),
                                PopupMenuItem(
                                  child: Text(translate("Multi Select")),
                                  value: "multi_select",
                                ),
                                PopupMenuItem(
                                  child: Text(translate("Properties")),
                                  value: "properties",
                                  enabled: false,
                                )
                              ];
                            },
                            onSelected: (v) {
                              if (v == "delete") {
                                final items = SelectedItems(isLocal: isLocal);
                                items.add(entries[index]);
                                controller.removeAction(items);
                              } else if (v == "multi_select") {
                                _selectedItems.clear();
                                widget.selectMode.toggle(isLocal);
                                setState(() {});
                              }
                            }),
                onTap: () {
                  if (showCheckBox) {
                    if (selected) {
                      _selectedItems.remove(entries[index]);
                    } else {
                      _selectedItems.add(entries[index]);
                    }
                    setState(() {});
                    return;
                  }
                  if (entries[index].isDirectory || entries[index].isDrive) {
                    controller.openDirectory(entries[index].path);
                  } else {
                    // Perform file-related tasks.
                  }
                },
                onLongPress: entries[index].isDrive
                    ? null
                    : () {
                        _selectedItems.clear();
                        widget.selectMode.toggle(isLocal);
                        if (widget.selectMode.value != SelectMode.none) {
                          _selectedItems.add(entries[index]);
                        }
                        setState(() {});
                      },
              ),
            );
          },
        );
      }))
    ]);
  }

  void breadCrumbScrollToEnd() {
    Future.delayed(Duration(milliseconds: 200), () {
      if (_breadCrumbScroller.hasClients) {
        _breadCrumbScroller.animateTo(
            _breadCrumbScroller.position.maxScrollExtent,
            duration: Duration(milliseconds: 200),
            curve: Curves.fastLinearToSlowEaseIn);
      }
    });
  }

  Widget headTools() => Container(
          child: Row(
        children: [
          Expanded(child: Obx(() {
            final home = controller.options.value.home;
            final isWindows = controller.options.value.isWindows;
            return BreadCrumb(
              items: getPathBreadCrumbItems(controller.shortPath, isWindows,
                  () => controller.goToHomeDirectory(), (list) {
                var path = "";
                if (home.startsWith(list[0])) {
                  // absolute path
                  for (var item in list) {
                    path = PathUtil.join(path, item, isWindows);
                  }
                } else {
                  path += home;
                  for (var item in list) {
                    path = PathUtil.join(path, item, isWindows);
                  }
                }
                controller.openDirectory(path);
              }),
              divider: Icon(Icons.chevron_right),
              overflow: ScrollableOverflow(controller: _breadCrumbScroller),
            );
          })),
          Row(
            children: [
              IconButton(
                icon: Icon(Icons.arrow_back),
                onPressed: controller.goBack,
              ),
              IconButton(
                icon: Icon(Icons.arrow_upward),
                onPressed: controller.goToParentDirectory,
              ),
              PopupMenuButton<SortBy>(
                  tooltip: "",
                  icon: Icon(Icons.sort),
                  itemBuilder: (context) {
                    return SortBy.values
                        .map((e) => PopupMenuItem(
                              child: Text(translate(e.toString())),
                              value: e,
                            ))
                        .toList();
                  },
                  onSelected: controller.changeSortStyle),
            ],
          )
        ],
      ));

  Widget listTail() => Obx(() => Container(
        height: 100,
        child: Column(
          children: [
            Padding(
              padding: EdgeInsets.fromLTRB(30, 5, 30, 0),
              child: Text(
                controller.directory.value.path,
                style: TextStyle(color: MyTheme.darkGray),
              ),
            ),
            Padding(
              padding: EdgeInsets.all(2),
              child: Text(
                "${translate("Total")}: ${controller.directory.value.entries.length} ${translate("items")}",
                style: TextStyle(color: MyTheme.darkGray),
              ),
            )
          ],
        ),
      ));

  List<BreadCrumbItem> getPathBreadCrumbItems(String shortPath, bool isWindows,
      void Function() onHome, void Function(List<String>) onPressed) {
    final list = PathUtil.split(shortPath, isWindows);
    final breadCrumbList = [
      BreadCrumbItem(
          content: IconButton(
        icon: Icon(Icons.home_filled),
        onPressed: onHome,
      ))
    ];
    breadCrumbList.addAll(list.asMap().entries.map((e) => BreadCrumbItem(
        content: TextButton(
            child: Text(e.value),
            style:
                ButtonStyle(minimumSize: MaterialStateProperty.all(Size(0, 0))),
            onPressed: () => onPressed(list.sublist(0, e.key + 1))))));
    return breadCrumbList;
  }
}

class BottomSheetBody extends StatelessWidget {
  BottomSheetBody(
      {required this.leading,
      required this.title,
      required this.text,
      this.onCanceled,
      this.actions});

  final Widget leading;
  final String title;
  final String text;
  final VoidCallback? onCanceled;
  final List<IconButton>? actions;

  @override
  BottomSheet build(BuildContext context) {
    // ignore: no_leading_underscores_for_local_identifiers
    final _actions = actions ?? [];
    return BottomSheet(
      builder: (BuildContext context) {
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
                      leading,
                      SizedBox(width: 16),
                      Column(
                        mainAxisAlignment: MainAxisAlignment.center,
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(title, style: TextStyle(fontSize: 18)),
                          Text(text,
                              style: TextStyle(fontSize: 14)) // TODO color
                        ],
                      )
                    ],
                  ),
                  Row(children: () {
                    _actions.add(IconButton(
                      icon: Icon(Icons.cancel_outlined),
                      onPressed: onCanceled,
                    ));
                    return _actions;
                  }())
                ],
              ),
            ));
      },
      onClosing: () {},
      // backgroundColor: MyTheme.grayBg,
      enableDrag: false,
    );
  }
}
