import 'dart:async';
import 'dart:io';
import 'dart:math';

import 'package:flutter_hbb/desktop/widgets/dragable_divider.dart';
import 'package:percent_indicator/percent_indicator.dart';
import 'package:desktop_drop/desktop_drop.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_breadcrumb/flutter_breadcrumb.dart';
import 'package:flutter_hbb/desktop/widgets/list_search_action_listener.dart';
import 'package:flutter_hbb/desktop/widgets/menu_button.dart';
import 'package:flutter_hbb/desktop/widgets/tabbar_widget.dart';
import 'package:flutter_hbb/models/file_model.dart';
import 'package:flutter_svg/flutter_svg.dart';
import 'package:get/get.dart';
import 'package:wakelock/wakelock.dart';

import '../../consts.dart';
import '../../desktop/widgets/material_mod_popup_menu.dart' as mod_menu;
import '../../common.dart';
import '../../models/model.dart';
import '../../models/platform_model.dart';
import '../widgets/popup_menu.dart';

/// status of location bar
enum LocationStatus {
  /// normal bread crumb bar
  bread,

  /// show path text field
  pathLocation,

  /// show file search bar text field
  fileSearchBar
}

/// The status of currently focused scope of the mouse
enum MouseFocusScope {
  /// Mouse is in local field.
  local,

  /// Mouse is in remote field.
  remote,

  /// Mouse is not in local field, remote neither.
  none
}

class FileManagerPage extends StatefulWidget {
  const FileManagerPage({Key? key, required this.id, this.forceRelay})
      : super(key: key);
  final String id;
  final bool? forceRelay;

  @override
  State<StatefulWidget> createState() => _FileManagerPageState();
}

class _FileManagerPageState extends State<FileManagerPage>
    with AutomaticKeepAliveClientMixin {
  final _mouseFocusScope = Rx<MouseFocusScope>(MouseFocusScope.none);

  final _dropMaskVisible = false.obs; // TODO impl drop mask
  final _overlayKeyState = OverlayKeyState();

  late FFI _ffi;

  FileModel get model => _ffi.fileModel;
  JobController get jobController => model.jobController;

  @override
  void initState() {
    super.initState();
    _ffi = FFI();
    _ffi.start(widget.id, isFileTransfer: true, forceRelay: widget.forceRelay);
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _ffi.dialogManager
          .showLoading(translate('Connecting...'), onCancel: closeConnection);
    });
    Get.put(_ffi, tag: 'ft_${widget.id}');
    if (!Platform.isLinux) {
      Wakelock.enable();
    }
    debugPrint("File manager page init success with id ${widget.id}");
    _ffi.dialogManager.setOverlayState(_overlayKeyState);
  }

  @override
  void dispose() {
    model.close().whenComplete(() {
      _ffi.close();
      _ffi.dialogManager.dismissAll();
      if (!Platform.isLinux) {
        Wakelock.disable();
      }
      Get.delete<FFI>(tag: 'ft_${widget.id}');
    });
    super.dispose();
  }

  @override
  bool get wantKeepAlive => true;

  @override
  Widget build(BuildContext context) {
    super.build(context);
    return Overlay(key: _overlayKeyState.key, initialEntries: [
      OverlayEntry(builder: (_) {
        return Scaffold(
          backgroundColor: Theme.of(context).scaffoldBackgroundColor,
          body: Row(
            children: [
              Flexible(
                  flex: 3,
                  child: dropArea(FileManagerView(
                      model.localController, _ffi, _mouseFocusScope))),
              Flexible(
                  flex: 3,
                  child: dropArea(FileManagerView(
                      model.remoteController, _ffi, _mouseFocusScope))),
              Flexible(flex: 2, child: statusList())
            ],
          ),
        );
      })
    ]);
  }

  Widget dropArea(FileManagerView fileView) {
    return DropTarget(
        onDragDone: (detail) =>
            handleDragDone(detail, fileView.controller.isLocal),
        onDragEntered: (enter) {
          _dropMaskVisible.value = true;
        },
        onDragExited: (exit) {
          _dropMaskVisible.value = false;
        },
        child: fileView);
  }

  Widget generateCard(Widget child) {
    return Container(
      decoration: BoxDecoration(
        color: Theme.of(context).cardColor,
        borderRadius: BorderRadius.all(
          Radius.circular(15.0),
        ),
      ),
      child: child,
    );
  }

  /// transfer status list
  /// watch transfer status
  Widget statusList() {
    statusListView(List<JobProgress> jobs) => ListView.builder(
          controller: ScrollController(),
          itemBuilder: (BuildContext context, int index) {
            final item = jobs[index];
            return Padding(
              padding: const EdgeInsets.only(bottom: 5),
              child: generateCard(
                Column(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Row(
                      crossAxisAlignment: CrossAxisAlignment.center,
                      children: [
                        Transform.rotate(
                          angle: item.isRemoteToLocal ? pi : 0,
                          child: SvgPicture.asset(
                            "assets/arrow.svg",
                            color: Theme.of(context).tabBarTheme.labelColor,
                          ),
                        ).paddingOnly(left: 15),
                        const SizedBox(
                          width: 16.0,
                        ),
                        Expanded(
                          child: Column(
                            mainAxisSize: MainAxisSize.min,
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                              Tooltip(
                                waitDuration: Duration(milliseconds: 500),
                                message: item.jobName,
                                child: Text(
                                  item.fileName,
                                  maxLines: 1,
                                  overflow: TextOverflow.ellipsis,
                                ).paddingSymmetric(vertical: 10),
                              ),
                              Text(
                                '${translate("Total")} ${readableFileSize(item.totalSize.toDouble())}',
                                style: TextStyle(
                                  fontSize: 12,
                                  color: MyTheme.darkGray,
                                ),
                              ),
                              Offstage(
                                offstage: item.state != JobState.inProgress,
                                child: Text(
                                  '${translate("Speed")} ${readableFileSize(item.speed)}/s',
                                  style: TextStyle(
                                    fontSize: 12,
                                    color: MyTheme.darkGray,
                                  ),
                                ),
                              ),
                              Offstage(
                                offstage: item.state == JobState.inProgress,
                                child: Text(
                                  translate(
                                    item.display(),
                                  ),
                                  style: TextStyle(
                                    fontSize: 12,
                                    color: MyTheme.darkGray,
                                  ),
                                ),
                              ),
                              Offstage(
                                offstage: item.state != JobState.inProgress,
                                child: LinearPercentIndicator(
                                  padding: EdgeInsets.only(right: 15),
                                  animateFromLastPercent: true,
                                  center: Text(
                                    '${(item.finishedSize / item.totalSize * 100).toStringAsFixed(0)}%',
                                  ),
                                  barRadius: Radius.circular(15),
                                  percent: item.finishedSize / item.totalSize,
                                  progressColor: MyTheme.accent,
                                  backgroundColor: Theme.of(context).hoverColor,
                                  lineHeight: kDesktopFileTransferRowHeight,
                                ).paddingSymmetric(vertical: 15),
                              ),
                            ],
                          ),
                        ),
                        Row(
                          mainAxisAlignment: MainAxisAlignment.end,
                          children: [
                            Offstage(
                              offstage: item.state != JobState.paused,
                              child: MenuButton(
                                onPressed: () {
                                  jobController.resumeJob(item.id);
                                },
                                child: SvgPicture.asset(
                                  "assets/refresh.svg",
                                  color: Colors.white,
                                ),
                                color: MyTheme.accent,
                                hoverColor: MyTheme.accent80,
                              ),
                            ),
                            MenuButton(
                              padding: EdgeInsets.only(right: 15),
                              child: SvgPicture.asset(
                                "assets/close.svg",
                                color: Colors.white,
                              ),
                              onPressed: () {
                                jobController.jobTable.removeAt(index);
                                jobController.cancelJob(item.id);
                              },
                              color: MyTheme.accent,
                              hoverColor: MyTheme.accent80,
                            ),
                          ],
                        ),
                      ],
                    ),
                  ],
                ).paddingSymmetric(vertical: 10),
              ),
            );
          },
          itemCount: jobController.jobTable.length,
        );

    return PreferredSize(
      preferredSize: const Size(200, double.infinity),
      child: Container(
          margin: const EdgeInsets.only(top: 16.0, bottom: 16.0, right: 16.0),
          padding: const EdgeInsets.all(8.0),
          child: Obx(
            () => jobController.jobTable.isEmpty
                ? generateCard(
                    Center(
                      child: Column(
                        mainAxisAlignment: MainAxisAlignment.center,
                        children: [
                          SvgPicture.asset(
                            "assets/transfer.svg",
                            color: Theme.of(context).tabBarTheme.labelColor,
                            height: 40,
                          ).paddingOnly(bottom: 10),
                          Text(
                            translate("No transfers in progress"),
                            textAlign: TextAlign.center,
                            textScaleFactor: 1.20,
                            style: TextStyle(
                                color:
                                    Theme.of(context).tabBarTheme.labelColor),
                          ),
                        ],
                      ),
                    ),
                  )
                : statusListView(jobController.jobTable),
          )),
    );
  }

  void handleDragDone(DropDoneDetails details, bool isLocal) {
    if (isLocal) {
      // ignore local
      return;
    }
    final items = SelectedItems(isLocal: false);
    for (var file in details.files) {
      final f = File(file.path);
      items.add(Entry()
        ..path = file.path
        ..name = file.name
        ..size = FileSystemEntity.isDirectorySync(f.path) ? 0 : f.lengthSync());
    }
    final otherSideData = model.localController.directoryData();
    model.remoteController.sendFiles(items, otherSideData);
  }
}

class FileManagerView extends StatefulWidget {
  final FileController controller;
  final FFI _ffi;
  final Rx<MouseFocusScope> _mouseFocusScope;

  FileManagerView(this.controller, this._ffi, this._mouseFocusScope);

  @override
  State<StatefulWidget> createState() => _FileManagerViewState();
}

class _FileManagerViewState extends State<FileManagerView> {
  final _locationStatus = LocationStatus.bread.obs;
  final _locationNode = FocusNode();
  final _locationBarKey = GlobalKey();
  final _searchText = "".obs;
  final _breadCrumbScroller = ScrollController();
  final _keyboardNode = FocusNode();
  final _listSearchBuffer = TimeoutStringBuffer();
  final _nameColWidth = kDesktopFileTransferNameColWidth.obs;
  final _modifiedColWidth = kDesktopFileTransferModifiedColWidth.obs;
  final _fileListScrollController = ScrollController();

  /// [_lastClickTime], [_lastClickEntry] help to handle double click
  var _lastClickTime =
      DateTime.now().millisecondsSinceEpoch - bind.getDoubleClickTime() - 1000;
  Entry? _lastClickEntry;

  FileController get controller => widget.controller;
  bool get isLocal => widget.controller.isLocal;
  FFI get _ffi => widget._ffi;
  SelectedItems get selectedItems => controller.selectedItems;

  @override
  void initState() {
    super.initState();
    // register location listener
    _locationNode.addListener(onLocationFocusChanged);
    controller.directory.listen((e) => breadCrumbScrollToEnd());
  }

  @override
  void dispose() {
    _locationNode.removeListener(onLocationFocusChanged);
    _locationNode.dispose();
    _keyboardNode.dispose();
    _breadCrumbScroller.dispose();
    _fileListScrollController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Container(
      margin: const EdgeInsets.all(16.0),
      padding: const EdgeInsets.all(8.0),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          headTools(),
          Expanded(
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Expanded(
                    child: MouseRegion(
                  onEnter: (evt) {
                    widget._mouseFocusScope.value = isLocal
                        ? MouseFocusScope.local
                        : MouseFocusScope.remote;
                    _keyboardNode.requestFocus();
                  },
                  onExit: (evt) =>
                      widget._mouseFocusScope.value = MouseFocusScope.none,
                  child: _buildFileList(context, _fileListScrollController),
                ))
              ],
            ),
          ),
        ],
      ),
    );
  }

  void onLocationFocusChanged() {
    debugPrint("focus changed on local");
    if (_locationNode.hasFocus) {
      // ignore
    } else {
      // lost focus, change to bread
      if (_locationStatus.value != LocationStatus.fileSearchBar) {
        _locationStatus.value = LocationStatus.bread;
      }
    }
  }

  Widget headTools() {
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
                          decoration: BoxDecoration(
                            borderRadius: BorderRadius.all(Radius.circular(8)),
                            color: MyTheme.accent,
                          ),
                          padding: EdgeInsets.all(8.0),
                          child: FutureBuilder<String>(
                              future: bind.sessionGetPlatform(
                                  id: _ffi.id, isRemote: !isLocal),
                              builder: (context, snapshot) {
                                if (snapshot.hasData &&
                                    snapshot.data!.isNotEmpty) {
                                  return getPlatformImage('${snapshot.data}');
                                } else {
                                  return CircularProgressIndicator(
                                    color: Theme.of(context)
                                        .tabBarTheme
                                        .labelColor,
                                  );
                                }
                              })),
                      Text(isLocal
                              ? translate("Local Computer")
                              : translate("Remote Computer"))
                          .marginOnly(left: 8.0)
                    ],
                  ),
                  preferredSize: Size(double.infinity, 70))
              .paddingOnly(bottom: 15),
          // buttons
          Row(
            children: [
              Row(
                children: [
                  MenuButton(
                    padding: EdgeInsets.only(
                      right: 3,
                    ),
                    child: RotatedBox(
                      quarterTurns: 2,
                      child: SvgPicture.asset(
                        "assets/arrow.svg",
                        color: Theme.of(context).tabBarTheme.labelColor,
                      ),
                    ),
                    color: Theme.of(context).cardColor,
                    hoverColor: Theme.of(context).hoverColor,
                    onPressed: () {
                      selectedItems.clear();
                      controller.goBack();
                    },
                  ),
                  MenuButton(
                    child: RotatedBox(
                      quarterTurns: 3,
                      child: SvgPicture.asset(
                        "assets/arrow.svg",
                        color: Theme.of(context).tabBarTheme.labelColor,
                      ),
                    ),
                    color: Theme.of(context).cardColor,
                    hoverColor: Theme.of(context).hoverColor,
                    onPressed: () {
                      selectedItems.clear();
                      controller.goToParentDirectory();
                    },
                  ),
                ],
              ),
              Expanded(
                child: Padding(
                  padding: const EdgeInsets.symmetric(horizontal: 3.0),
                  child: Container(
                    decoration: BoxDecoration(
                      color: Theme.of(context).cardColor,
                      borderRadius: BorderRadius.all(
                        Radius.circular(8.0),
                      ),
                    ),
                    child: Padding(
                      padding: EdgeInsets.symmetric(vertical: 2.5),
                      child: GestureDetector(
                        onTap: () {
                          _locationStatus.value =
                              _locationStatus.value == LocationStatus.bread
                                  ? LocationStatus.pathLocation
                                  : LocationStatus.bread;
                          Future.delayed(Duration.zero, () {
                            if (_locationStatus.value ==
                                LocationStatus.pathLocation) {
                              _locationNode.requestFocus();
                            }
                          });
                        },
                        child: Obx(
                          () => Container(
                            child: Row(
                              children: [
                                Expanded(
                                    child: _locationStatus.value ==
                                            LocationStatus.bread
                                        ? buildBread()
                                        : buildPathLocation()),
                              ],
                            ),
                          ),
                        ),
                      ),
                    ),
                  ),
                ),
              ),
              Obx(() {
                switch (_locationStatus.value) {
                  case LocationStatus.bread:
                    return MenuButton(
                      onPressed: () {
                        _locationStatus.value = LocationStatus.fileSearchBar;
                        Future.delayed(
                            Duration.zero, () => _locationNode.requestFocus());
                      },
                      child: SvgPicture.asset(
                        "assets/search.svg",
                        color: Theme.of(context).tabBarTheme.labelColor,
                      ),
                      color: Theme.of(context).cardColor,
                      hoverColor: Theme.of(context).hoverColor,
                    );
                  case LocationStatus.pathLocation:
                    return MenuButton(
                      onPressed: null,
                      child: SvgPicture.asset(
                        "assets/close.svg",
                        color: Theme.of(context).tabBarTheme.labelColor,
                      ),
                      color: Theme.of(context).disabledColor,
                      hoverColor: Theme.of(context).hoverColor,
                    );
                  case LocationStatus.fileSearchBar:
                    return MenuButton(
                      onPressed: () {
                        onSearchText("", isLocal);
                        _locationStatus.value = LocationStatus.bread;
                      },
                      child: SvgPicture.asset(
                        "assets/close.svg",
                        color: Theme.of(context).tabBarTheme.labelColor,
                      ),
                      color: Theme.of(context).cardColor,
                      hoverColor: Theme.of(context).hoverColor,
                    );
                }
              }),
              MenuButton(
                padding: EdgeInsets.only(
                  left: 3,
                ),
                onPressed: () {
                  controller.refresh();
                },
                child: SvgPicture.asset(
                  "assets/refresh.svg",
                  color: Theme.of(context).tabBarTheme.labelColor,
                ),
                color: Theme.of(context).cardColor,
                hoverColor: Theme.of(context).hoverColor,
              ),
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
                    MenuButton(
                      padding: EdgeInsets.only(
                        right: 3,
                      ),
                      onPressed: () {
                        controller.goToHomeDirectory();
                      },
                      child: SvgPicture.asset(
                        "assets/home.svg",
                        color: Theme.of(context).tabBarTheme.labelColor,
                      ),
                      color: Theme.of(context).cardColor,
                      hoverColor: Theme.of(context).hoverColor,
                    ),
                    MenuButton(
                      onPressed: () {
                        final name = TextEditingController();
                        _ffi.dialogManager.show((setState, close) {
                          submit() {
                            if (name.value.text.isNotEmpty) {
                              controller.createDir(PathUtil.join(
                                controller.directory.value.path,
                                name.value.text,
                                controller.options.value.isWindows,
                              ));
                              close();
                            }
                          }

                          cancel() => close(false);
                          return CustomAlertDialog(
                            title: Row(
                              mainAxisAlignment: MainAxisAlignment.center,
                              children: [
                                SvgPicture.asset("assets/folder_new.svg",
                                    color: MyTheme.accent),
                                Text(
                                  translate("Create Folder"),
                                ).paddingOnly(
                                  left: 10,
                                ),
                              ],
                            ),
                            content: Column(
                              mainAxisSize: MainAxisSize.min,
                              children: [
                                TextFormField(
                                  decoration: InputDecoration(
                                    labelText: translate(
                                      "Please enter the folder name",
                                    ),
                                  ),
                                  controller: name,
                                  autofocus: true,
                                ),
                              ],
                            ),
                            actions: [
                              dialogButton(
                                "Cancel",
                                icon: Icon(Icons.close_rounded),
                                onPressed: cancel,
                                isOutline: true,
                              ),
                              dialogButton(
                                "Ok",
                                icon: Icon(Icons.done_rounded),
                                onPressed: submit,
                              ),
                            ],
                            onSubmit: submit,
                            onCancel: cancel,
                          );
                        });
                      },
                      child: SvgPicture.asset(
                        "assets/folder_new.svg",
                        color: Theme.of(context).tabBarTheme.labelColor,
                      ),
                      color: Theme.of(context).cardColor,
                      hoverColor: Theme.of(context).hoverColor,
                    ),
                    Obx(() => MenuButton(
                          onPressed: SelectedItems.valid(selectedItems.items)
                              ? () async {
                                  await (controller
                                      .removeAction(selectedItems));
                                  selectedItems.clear();
                                }
                              : null,
                          child: SvgPicture.asset(
                            "assets/trash.svg",
                            color: Theme.of(context).tabBarTheme.labelColor,
                          ),
                          color: Theme.of(context).cardColor,
                          hoverColor: Theme.of(context).hoverColor,
                        )),
                    menu(isLocal: isLocal),
                  ],
                ),
              ),
              Obx(() => ElevatedButton.icon(
                    style: ButtonStyle(
                      padding: MaterialStateProperty.all<EdgeInsetsGeometry>(
                          isLocal
                              ? EdgeInsets.only(left: 10)
                              : EdgeInsets.only(right: 10)),
                      backgroundColor: MaterialStateProperty.all(
                        selectedItems.items.isEmpty
                            ? MyTheme.accent80
                            : MyTheme.accent,
                      ),
                    ),
                    onPressed: SelectedItems.valid(selectedItems.items)
                        ? () {
                            final otherSideData =
                                controller.getOtherSideDirectoryData();
                            controller.sendFiles(selectedItems, otherSideData);
                            selectedItems.clear();
                          }
                        : null,
                    icon: isLocal
                        ? Text(
                            translate('Send'),
                            textAlign: TextAlign.right,
                            style: TextStyle(
                              color: selectedItems.items.isEmpty
                                  ? Theme.of(context).brightness ==
                                          Brightness.light
                                      ? MyTheme.grayBg
                                      : MyTheme.darkGray
                                  : Colors.white,
                            ),
                          )
                        : RotatedBox(
                            quarterTurns: 2,
                            child: SvgPicture.asset(
                              "assets/arrow.svg",
                              color: selectedItems.items.isEmpty
                                  ? Theme.of(context).brightness ==
                                          Brightness.light
                                      ? MyTheme.grayBg
                                      : MyTheme.darkGray
                                  : Colors.white,
                              alignment: Alignment.bottomRight,
                            ),
                          ),
                    label: isLocal
                        ? SvgPicture.asset(
                            "assets/arrow.svg",
                            color: selectedItems.items.isEmpty
                                ? Theme.of(context).brightness ==
                                        Brightness.light
                                    ? MyTheme.grayBg
                                    : MyTheme.darkGray
                                : Colors.white,
                          )
                        : Text(
                            translate('Receive'),
                            style: TextStyle(
                              color: selectedItems.items.isEmpty
                                  ? Theme.of(context).brightness ==
                                          Brightness.light
                                      ? MyTheme.grayBg
                                      : MyTheme.darkGray
                                  : Colors.white,
                            ),
                          ),
                  )),
            ],
          ).marginOnly(top: 8.0)
        ],
      ),
    );
  }

  Widget menu({bool isLocal = false}) {
    var menuPos = RelativeRect.fill;

    final List<MenuEntryBase<String>> items = [
      MenuEntrySwitch<String>(
        switchType: SwitchType.scheckbox,
        text: translate("Show Hidden Files"),
        getter: () async {
          return controller.options.value.isWindows;
        },
        setter: (bool v) async {
          controller.toggleShowHidden();
        },
        padding: kDesktopMenuPadding,
        dismissOnClicked: true,
      ),
      MenuEntryButton(
          childBuilder: (style) => Text(translate("Select All"), style: style),
          proc: () => setState(() =>
              selectedItems.selectAll(controller.directory.value.entries)),
          padding: kDesktopMenuPadding,
          dismissOnClicked: true),
      MenuEntryButton(
          childBuilder: (style) =>
              Text(translate("Unselect All"), style: style),
          proc: () => selectedItems.clear(),
          padding: kDesktopMenuPadding,
          dismissOnClicked: true)
    ];

    return Listener(
      onPointerDown: (e) {
        final x = e.position.dx;
        final y = e.position.dy;
        menuPos = RelativeRect.fromLTRB(x, y, x, y);
      },
      child: MenuButton(
        onPressed: () => mod_menu.showMenu(
          context: context,
          position: menuPos,
          items: items
              .map(
                (e) => e.build(
                  context,
                  MenuConfig(
                      commonColor: CustomPopupMenuTheme.commonColor,
                      height: CustomPopupMenuTheme.height,
                      dividerHeight: CustomPopupMenuTheme.dividerHeight),
                ),
              )
              .expand((i) => i)
              .toList(),
          elevation: 8,
        ),
        child: SvgPicture.asset(
          "assets/dots.svg",
          color: Theme.of(context).tabBarTheme.labelColor,
        ),
        color: Theme.of(context).cardColor,
        hoverColor: Theme.of(context).hoverColor,
      ),
    );
  }

  Widget _buildFileList(
      BuildContext context, ScrollController scrollController) {
    final fd = controller.directory.value;
    final entries = fd.entries;

    return ListSearchActionListener(
      node: _keyboardNode,
      buffer: _listSearchBuffer,
      onNext: (buffer) {
        debugPrint("searching next for $buffer");
        assert(buffer.length == 1);
        assert(selectedItems.items.length <= 1);
        var skipCount = 0;
        if (selectedItems.items.isNotEmpty) {
          final index = entries.indexOf(selectedItems.items.first);
          if (index < 0) {
            return;
          }
          skipCount = index + 1;
        }
        var searchResult = entries
            .skip(skipCount)
            .where((element) => element.name.toLowerCase().startsWith(buffer));
        if (searchResult.isEmpty) {
          // cannot find next, lets restart search from head
          debugPrint("restart search from head");
          searchResult = entries.where(
              (element) => element.name.toLowerCase().startsWith(buffer));
        }
        if (searchResult.isEmpty) {
          selectedItems.clear();
          return;
        }
        _jumpToEntry(isLocal, searchResult.first, scrollController,
            kDesktopFileTransferRowHeight);
      },
      onSearch: (buffer) {
        debugPrint("searching for $buffer");
        final selectedEntries = selectedItems;
        final searchResult = entries
            .where((element) => element.name.toLowerCase().startsWith(buffer));
        selectedEntries.clear();
        if (searchResult.isEmpty) {
          selectedItems.clear();
          return;
        }
        _jumpToEntry(isLocal, searchResult.first, scrollController,
            kDesktopFileTransferRowHeight);
      },
      child: Obx(() {
        final entries = controller.directory.value.entries;
        final filteredEntries = _searchText.isNotEmpty
            ? entries.where((element) {
                return element.name.contains(_searchText.value);
              }).toList(growable: false)
            : entries;
        final rows = filteredEntries.map((entry) {
          final sizeStr =
              entry.isFile ? readableFileSize(entry.size.toDouble()) : "";
          final lastModifiedStr = entry.isDrive
              ? " "
              : "${entry.lastModified().toString().replaceAll(".000", "")}   ";
          return Padding(
            padding: EdgeInsets.symmetric(vertical: 1),
            child: Obx(() => Container(
                decoration: BoxDecoration(
                  color: selectedItems.items.contains(entry)
                      ? Theme.of(context).hoverColor
                      : Theme.of(context).cardColor,
                  borderRadius: BorderRadius.all(
                    Radius.circular(5.0),
                  ),
                ),
                key: ValueKey(entry.name),
                height: kDesktopFileTransferRowHeight,
                child: Column(
                  mainAxisAlignment: MainAxisAlignment.spaceAround,
                  children: [
                    Expanded(
                      child: InkWell(
                        child: Row(
                          children: [
                            GestureDetector(
                              child: Obx(
                                () => Container(
                                    width: _nameColWidth.value,
                                    child: Tooltip(
                                      waitDuration: Duration(milliseconds: 500),
                                      message: entry.name,
                                      child: Row(children: [
                                        entry.isDrive
                                            ? Image(
                                                    image: iconHardDrive,
                                                    fit: BoxFit.scaleDown,
                                                    color: Theme.of(context)
                                                        .iconTheme
                                                        .color
                                                        ?.withOpacity(0.7))
                                                .paddingAll(4)
                                            : SvgPicture.asset(
                                                entry.isFile
                                                    ? "assets/file.svg"
                                                    : "assets/folder.svg",
                                                color: Theme.of(context)
                                                    .tabBarTheme
                                                    .labelColor,
                                              ),
                                        Expanded(
                                            child: Text(entry.name.nonBreaking,
                                                overflow:
                                                    TextOverflow.ellipsis))
                                      ]),
                                    )),
                              ),
                              onTap: () {
                                final items = selectedItems;
                                // handle double click
                                if (_checkDoubleClick(entry)) {
                                  controller.openDirectory(entry.path);
                                  items.clear();
                                  return;
                                }
                                _onSelectedChanged(
                                    items, filteredEntries, entry, isLocal);
                              },
                            ),
                            SizedBox(
                              width: 2.0,
                            ),
                            GestureDetector(
                              child: Obx(
                                () => SizedBox(
                                  width: _modifiedColWidth.value,
                                  child: Tooltip(
                                      waitDuration: Duration(milliseconds: 500),
                                      message: lastModifiedStr,
                                      child: Text(
                                        lastModifiedStr,
                                        overflow: TextOverflow.ellipsis,
                                        style: TextStyle(
                                          fontSize: 12,
                                          color: MyTheme.darkGray,
                                        ),
                                      )),
                                ),
                              ),
                            ),
                            // Divider from header.
                            SizedBox(
                              width: 2.0,
                            ),
                            Expanded(
                              // width: 100,
                              child: GestureDetector(
                                child: Tooltip(
                                  waitDuration: Duration(milliseconds: 500),
                                  message: sizeStr,
                                  child: Text(
                                    sizeStr,
                                    overflow: TextOverflow.ellipsis,
                                    style: TextStyle(
                                        fontSize: 10, color: MyTheme.darkGray),
                                  ),
                                ),
                              ),
                            ),
                          ],
                        ),
                      ),
                    ),
                  ],
                ))),
          );
        }).toList(growable: false);

        return Column(
          children: [
            // Header
            Row(
              children: [
                Expanded(child: _buildFileBrowserHeader(context)),
              ],
            ),
            // Body
            Expanded(
              child: ListView.builder(
                controller: scrollController,
                itemExtent: kDesktopFileTransferRowHeight,
                itemBuilder: (context, index) {
                  return rows[index];
                },
                itemCount: rows.length,
              ),
            ),
          ],
        );
      }),
    );
  }

  onSearchText(String searchText, bool isLocal) {
    selectedItems.clear();
    _searchText.value = searchText;
  }

  void _jumpToEntry(bool isLocal, Entry entry,
      ScrollController scrollController, double rowHeight) {
    final entries = controller.directory.value.entries;
    final index = entries.indexOf(entry);
    if (index == -1) {
      debugPrint("entry is not valid: ${entry.path}");
    }
    final selectedEntries = selectedItems;
    final searchResult = entries.where((element) => element == entry);
    selectedEntries.clear();
    if (searchResult.isEmpty) {
      return;
    }
    final offset = min(
        max(scrollController.position.minScrollExtent,
            entries.indexOf(searchResult.first) * rowHeight),
        scrollController.position.maxScrollExtent);
    scrollController.jumpTo(offset);
    selectedEntries.add(searchResult.first);
    debugPrint("focused on ${searchResult.first.name}");
  }

  void _onSelectedChanged(SelectedItems selectedItems, List<Entry> entries,
      Entry entry, bool isLocal) {
    final isCtrlDown = RawKeyboard.instance.keysPressed
        .contains(LogicalKeyboardKey.controlLeft);
    final isShiftDown =
        RawKeyboard.instance.keysPressed.contains(LogicalKeyboardKey.shiftLeft);
    if (isCtrlDown) {
      if (selectedItems.items.contains(entry)) {
        selectedItems.remove(entry);
      } else {
        selectedItems.add(entry);
      }
    } else if (isShiftDown) {
      final List<int> indexGroup = [];
      for (var selected in selectedItems.items) {
        indexGroup.add(entries.indexOf(selected));
      }
      indexGroup.add(entries.indexOf(entry));
      indexGroup.removeWhere((e) => e == -1);
      final maxIndex = indexGroup.reduce(max);
      final minIndex = indexGroup.reduce(min);
      selectedItems.clear();
      entries
          .getRange(minIndex, maxIndex + 1)
          .forEach((e) => selectedItems.add(e));
    } else {
      selectedItems.clear();
      selectedItems.add(entry);
    }
    setState(() {});
  }

  bool _checkDoubleClick(Entry entry) {
    final current = DateTime.now().millisecondsSinceEpoch;
    final elapsed = current - _lastClickTime;
    _lastClickTime = current;
    if (_lastClickEntry == entry) {
      if (elapsed < bind.getDoubleClickTime()) {
        return true;
      }
    } else {
      _lastClickEntry = entry;
    }
    return false;
  }

  Widget _buildFileBrowserHeader(BuildContext context) {
    final padding = EdgeInsets.all(1.0);
    return SizedBox(
      height: kDesktopFileTransferHeaderHeight,
      child: Row(
        children: [
          Obx(
            () => headerItemFunc(
                _nameColWidth.value, SortBy.name, translate("Name")),
          ),
          DraggableDivider(
            axis: Axis.vertical,
            onPointerMove: (dx) {
              _nameColWidth.value += dx;
              _nameColWidth.value = min(kDesktopFileTransferMaximumWidth,
                  max(kDesktopFileTransferMinimumWidth, _nameColWidth.value));
            },
            padding: padding,
          ),
          Obx(
            () => headerItemFunc(_modifiedColWidth.value, SortBy.modified,
                translate("Modified")),
          ),
          DraggableDivider(
              axis: Axis.vertical,
              onPointerMove: (dx) {
                _modifiedColWidth.value += dx;
                _modifiedColWidth.value = min(
                    kDesktopFileTransferMaximumWidth,
                    max(kDesktopFileTransferMinimumWidth,
                        _modifiedColWidth.value));
              },
              padding: padding),
          Expanded(child: headerItemFunc(null, SortBy.size, translate("Size")))
        ],
      ),
    );
  }

  Widget headerItemFunc(double? width, SortBy sortBy, String name) {
    final headerTextStyle =
        Theme.of(context).dataTableTheme.headingTextStyle ?? TextStyle();
    return ObxValue<Rx<bool?>>(
        (ascending) => InkWell(
              onTap: () {
                if (ascending.value == null) {
                  ascending.value = true;
                } else {
                  ascending.value = !ascending.value!;
                }
                controller.changeSortStyle(sortBy,
                    isLocal: isLocal, ascending: ascending.value!);
              },
              child: SizedBox(
                width: width,
                height: kDesktopFileTransferHeaderHeight,
                child: Row(
                  children: [
                    Flexible(
                      flex: 2,
                      child: Text(
                        name,
                        style: headerTextStyle,
                        overflow: TextOverflow.ellipsis,
                      ).marginSymmetric(horizontal: 4),
                    ),
                    Flexible(
                        flex: 1,
                        child: ascending.value != null
                            ? Icon(
                                ascending.value!
                                    ? Icons.keyboard_arrow_up_rounded
                                    : Icons.keyboard_arrow_down_rounded,
                              )
                            : const Offstage())
                  ],
                ),
              ),
            ), () {
      if (controller.sortBy.value == sortBy) {
        return controller.sortAscending.obs;
      } else {
        return Rx<bool?>(null);
      }
    }());
  }

  Widget buildBread() {
    final items = getPathBreadCrumbItems(isLocal, (list) {
      var path = "";
      for (var item in list) {
        path = PathUtil.join(path, item, controller.options.value.isWindows);
      }
      controller.openDirectory(path);
    });

    return items.isEmpty
        ? Offstage()
        : Row(
            key: _locationBarKey,
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
                Expanded(
                  child: Listener(
                    // handle mouse wheel
                    onPointerSignal: (e) {
                      if (e is PointerScrollEvent) {
                        final sc = _breadCrumbScroller;
                        final scale = Platform.isWindows ? 2 : 4;
                        sc.jumpTo(sc.offset + e.scrollDelta.dy / scale);
                      }
                    },
                    child: BreadCrumb(
                      items: items,
                      divider: const Icon(Icons.keyboard_arrow_right_rounded),
                      overflow: ScrollableOverflow(
                        controller: _breadCrumbScroller,
                      ),
                    ),
                  ),
                ),
                ActionIcon(
                  message: "",
                  icon: Icons.keyboard_arrow_down_rounded,
                  onTap: () async {
                    final renderBox = _locationBarKey.currentContext
                        ?.findRenderObject() as RenderBox;
                    _locationBarKey.currentContext?.size;

                    final size = renderBox.size;
                    final offset = renderBox.localToGlobal(Offset.zero);

                    final x = offset.dx;
                    final y = offset.dy + size.height + 1;

                    final isPeerWindows = controller.options.value.isWindows;
                    final List<MenuEntryBase> menuItems = [
                      MenuEntryButton(
                          childBuilder: (TextStyle? style) => isPeerWindows
                              ? buildWindowsThisPC(context, style)
                              : Text(
                                  '/',
                                  style: style,
                                ),
                          proc: () {
                            controller.openDirectory('/');
                          },
                          dismissOnClicked: true),
                      MenuEntryDivider()
                    ];
                    if (isPeerWindows) {
                      var loadingTag = "";
                      if (!isLocal) {
                        loadingTag = _ffi.dialogManager.showLoading("Waiting");
                      }
                      try {
                        final showHidden = controller.options.value.showHidden;
                        final fd = await controller.fileFetcher
                            .fetchDirectory("/", isLocal, showHidden);
                        for (var entry in fd.entries) {
                          menuItems.add(MenuEntryButton(
                              childBuilder: (TextStyle? style) =>
                                  Row(children: [
                                    Image(
                                        image: iconHardDrive,
                                        fit: BoxFit.scaleDown,
                                        color: Theme.of(context)
                                            .iconTheme
                                            .color
                                            ?.withOpacity(0.7)),
                                    SizedBox(width: 10),
                                    Text(
                                      entry.name,
                                      style: style,
                                    )
                                  ]),
                              proc: () {
                                controller.openDirectory('${entry.name}\\');
                              },
                              dismissOnClicked: true));
                        }
                        menuItems.add(MenuEntryDivider());
                      } catch (e) {
                        debugPrint("buildBread fetchDirectory err=$e");
                      } finally {
                        if (!isLocal) {
                          _ffi.dialogManager.dismissByTag(loadingTag);
                        }
                      }
                    }
                    mod_menu.showMenu(
                        context: context,
                        position: RelativeRect.fromLTRB(x, y, x, y),
                        elevation: 4,
                        items: menuItems
                            .map((e) => e.build(
                                context,
                                MenuConfig(
                                    commonColor:
                                        CustomPopupMenuTheme.commonColor,
                                    height: CustomPopupMenuTheme.height,
                                    dividerHeight:
                                        CustomPopupMenuTheme.dividerHeight,
                                    boxWidth: size.width)))
                            .expand((i) => i)
                            .toList());
                  },
                  iconSize: 20,
                )
              ]);
  }

  List<BreadCrumbItem> getPathBreadCrumbItems(
      bool isLocal, void Function(List<String>) onPressed) {
    final path = controller.directory.value.path;
    final breadCrumbList = List<BreadCrumbItem>.empty(growable: true);
    final isWindows = controller.options.value.isWindows;
    if (isWindows && path == '/') {
      breadCrumbList.add(BreadCrumbItem(
          content: TextButton(
                  child: buildWindowsThisPC(context),
                  style: ButtonStyle(
                      minimumSize: MaterialStateProperty.all(Size(0, 0))),
                  onPressed: () => onPressed(['/']))
              .marginSymmetric(horizontal: 4)));
    } else {
      final list = PathUtil.split(path, isWindows);
      breadCrumbList.addAll(
        list.asMap().entries.map(
              (e) => BreadCrumbItem(
                content: TextButton(
                  child: Text(e.value),
                  style: ButtonStyle(
                    minimumSize: MaterialStateProperty.all(
                      Size(0, 0),
                    ),
                  ),
                  onPressed: () => onPressed(
                    list.sublist(0, e.key + 1),
                  ),
                ).marginSymmetric(horizontal: 4),
              ),
            ),
      );
    }
    return breadCrumbList;
  }

  breadCrumbScrollToEnd() {
    Future.delayed(Duration(milliseconds: 200), () {
      if (_breadCrumbScroller.hasClients) {
        _breadCrumbScroller.animateTo(
            _breadCrumbScroller.position.maxScrollExtent,
            duration: Duration(milliseconds: 200),
            curve: Curves.fastLinearToSlowEaseIn);
      }
    });
  }

  Widget buildPathLocation() {
    final text = _locationStatus.value == LocationStatus.pathLocation
        ? controller.directory.value.path
        : _searchText.value;
    final textController = TextEditingController(text: text)
      ..selection = TextSelection.collapsed(offset: text.length);
    return Row(
      children: [
        SvgPicture.asset(
          _locationStatus.value == LocationStatus.pathLocation
              ? "assets/folder.svg"
              : "assets/search.svg",
          color: Theme.of(context).tabBarTheme.labelColor,
        ),
        Expanded(
          child: TextField(
            focusNode: _locationNode,
            decoration: InputDecoration(
              border: InputBorder.none,
              isDense: true,
              prefix: Padding(
                padding: EdgeInsets.only(left: 4.0),
              ),
            ),
            controller: textController,
            onSubmitted: (path) {
              controller.openDirectory(path);
            },
            onChanged: _locationStatus.value == LocationStatus.fileSearchBar
                ? (searchText) => onSearchText(searchText, isLocal)
                : null,
          ),
        )
      ],
    );
  }

  // openDirectory(String path, {bool isLocal = false}) {
  //   model.openDirectory(path, isLocal: isLocal);
  // }
}

Widget buildWindowsThisPC(BuildContext context, [TextStyle? textStyle]) {
  final color = Theme.of(context).iconTheme.color?.withOpacity(0.7);
  return Row(children: [
    Icon(Icons.computer, size: 20, color: color),
    SizedBox(width: 10),
    Text(translate('This PC'), style: textStyle)
  ]);
}
