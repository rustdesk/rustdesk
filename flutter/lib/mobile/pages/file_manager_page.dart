import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_breadcrumb/flutter_breadcrumb.dart';
import 'package:flutter_hbb/models/file_model.dart';
import 'package:get/get.dart';
import 'package:toggle_switch/toggle_switch.dart';
import 'package:wakelock/wakelock.dart';

import '../../common.dart';
import '../widgets/dialog.dart';

class FileManagerPage extends StatefulWidget {
  FileManagerPage({Key? key, required this.id}) : super(key: key);
  final String id;

  @override
  State<StatefulWidget> createState() => _FileManagerPageState();
}

class _FileManagerPageState extends State<FileManagerPage> {
  final model = gFFI.fileModel;
  var showLocal = true;
  var isSelecting = false.obs;

  FileController get currentFileController =>
      showLocal ? model.localController : model.remoteController;
  FileDirectory get currentDir => currentFileController.directory.value;
  DirectoryOptions get currentOptions => currentFileController.options.value;

  @override
  void initState() {
    super.initState();
    gFFI.start(widget.id, isFileTransfer: true);
    WidgetsBinding.instance.addPostFrameCallback((_) {
      gFFI.dialogManager
          .showLoading(translate('Connecting...'), onCancel: closeConnection);
    });
    gFFI.ffiModel.updateEventListener(widget.id);
    Wakelock.enable();
  }

  @override
  void dispose() {
    model.close().whenComplete(() {
      gFFI.close();
      gFFI.dialogManager.dismissAll();
      Wakelock.disable();
    });
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => WillPopScope(
      onWillPop: () async {
        if (isSelecting.value) {
          isSelecting.value = false;
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
                onPressed: () => clientClose(widget.id, gFFI.dialogManager)),
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
                    isSelecting.toggle();
                  } else if (v == "folder") {
                    final name = TextEditingController();
                    gFFI.dialogManager
                        .show((setState, close) => CustomAlertDialog(
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
                isSelecting: isSelecting,
                showCheckBox: showCheckBox())
            : FileManagerView(
                controller: model.remoteController,
                isSelecting: isSelecting,
                showCheckBox: showCheckBox()),
        bottomSheet: bottomSheet(),
      ));

  bool showCheckBox() {
    final selectedItems = getActiveSelectedItems();

    if (selectedItems != null) {
      return selectedItems.isLocal == showLocal;
    }
    return false;
  }

  Widget? bottomSheet() {
    return Obx(() {
      final selectedItems = getActiveSelectedItems();

      final localLabel = selectedItems?.isLocal == null
          ? ""
          : " [${selectedItems!.isLocal ? translate("Local") : translate("Remote")}]";

      if (isSelecting.value) {
        final selectedItemsLen =
            "${selectedItems?.items.length ?? 0} ${translate("items")}";
        if (selectedItems == null ||
            selectedItems.items.isEmpty ||
            showCheckBox()) {
          return BottomSheetBody(
              leading: Icon(Icons.check),
              title: translate("Selected"),
              text: selectedItemsLen + localLabel,
              onCanceled: () => isSelecting.toggle(),
              actions: [
                IconButton(
                  icon: Icon(Icons.compare_arrows),
                  onPressed: () => setState(() => showLocal = !showLocal),
                ),
                IconButton(
                  icon: Icon(Icons.delete_forever),
                  onPressed: selectedItems != null
                      ? () {
                          if (selectedItems.items.isNotEmpty) {
                            currentFileController.removeAction(selectedItems);
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
              onCanceled: () => isSelecting.toggle(),
              actions: [
                IconButton(
                  icon: Icon(Icons.compare_arrows),
                  onPressed: () => setState(() => showLocal = !showLocal),
                ),
                IconButton(
                  icon: Icon(Icons.paste),
                  onPressed: () {
                    isSelecting.toggle();
                    final otherSide = showLocal
                        ? model.remoteController
                        : model.localController;
                    final otherSideData = DirectoryData(
                        otherSide.directory.value, otherSide.options.value);
                    currentFileController.sendFiles(
                        selectedItems, otherSideData);
                  },
                )
              ]);
        }
      }

      final jobTable = model.jobController.jobTable;

      if (jobTable.isEmpty) {
        return Offstage();
      }

      switch (jobTable.last.state) {
        case JobState.inProgress:
          return Obx(() => BottomSheetBody(
                leading: CircularProgressIndicator(),
                title: translate("Waiting"),
                text:
                    "${translate("Speed")}:  ${readableFileSize(jobTable.last.speed)}/s",
                onCanceled: () =>
                    model.jobController.cancelJob(jobTable.last.id),
              ));
        case JobState.done:
          return Obx(() => BottomSheetBody(
                leading: Icon(Icons.check),
                title: "${translate("Successful")}!",
                text: jobTable.last.display(),
                onCanceled: () => jobTable.clear(),
              ));
        case JobState.error:
          return Obx(() => BottomSheetBody(
                leading: Icon(Icons.error),
                title: "${translate("Error")}!",
                text: "",
                onCanceled: () => jobTable.clear(),
              ));
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
  final RxBool isSelecting;
  final bool showCheckBox;

  FileManagerView(
      {required this.controller,
      required this.isSelecting,
      required this.showCheckBox});

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
            if (widget.isSelecting.value) {
              selected = _selectedItems.items.contains(entries[index]);
            }

            final sizeStr = entries[index].isFile
                ? readableFileSize(entries[index].size.toDouble())
                : "";
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
                    : widget.isSelecting.value && widget.showCheckBox
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
                                widget.isSelecting.toggle();
                              }
                            }),
                onTap: () {
                  if (widget.isSelecting.value && widget.showCheckBox) {
                    if (selected) {
                      _selectedItems.remove(entries[index]);
                    } else {
                      _selectedItems.add(entries[index]);
                    }
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
                        widget.isSelecting.toggle();
                        if (widget.isSelecting.value) {
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
