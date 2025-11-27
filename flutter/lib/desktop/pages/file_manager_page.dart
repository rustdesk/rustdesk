import 'dart:async';
import 'dart:io';
import 'dart:math';

import 'package:extended_text/extended_text.dart';
import 'package:flutter_hbb/common/widgets/dialog.dart';
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
import 'package:wakelock_plus/wakelock_plus.dart';
import 'package:flutter_hbb/web/dummy.dart'
    if (dart.library.html) 'package:flutter_hbb/web/web_unique.dart';

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
  FileManagerPage(
      {Key? key,
      required this.id,
      required this.password,
      required this.isSharedPassword,
      this.tabController,
      this.connToken,
      this.forceRelay})
      : super(key: key);
  final String id;
  final String? password;
  final bool? isSharedPassword;
  final bool? forceRelay;
  final String? connToken;
  final DesktopTabController? tabController;
  final SimpleWrapper<State<FileManagerPage>?> _lastState = SimpleWrapper(null);

  FFI get ffi => (_lastState.value! as _FileManagerPageState)._ffi;

  @override
  State<StatefulWidget> createState() {
    final state = _FileManagerPageState();
    _lastState.value = state;
    return state;
  }
}

class _FileManagerPageState extends State<FileManagerPage>
    with AutomaticKeepAliveClientMixin, WidgetsBindingObserver {
  final _mouseFocusScope = Rx<MouseFocusScope>(MouseFocusScope.none);

  final _dropMaskVisible = false.obs; // TODO impl drop mask
  final _overlayKeyState = OverlayKeyState();

  late FFI _ffi;

  FileModel get model => _ffi.fileModel;
  JobController get jobController => model.jobController;

  @override
  void initState() {
    super.initState();
    _ffi = FFI(null);
    _ffi.start(widget.id,
        isFileTransfer: true,
        password: widget.password,
        isSharedPassword: widget.isSharedPassword,
        connToken: widget.connToken,
        forceRelay: widget.forceRelay);
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _ffi.dialogManager
          .showLoading(translate('Connecting...'), onCancel: closeConnection);
    });
    Get.put<FFI>(_ffi, tag: 'ft_${widget.id}');
    if (!isLinux) {
      WakelockPlus.enable();
    }
    if (isWeb) {
      _ffi.ffiModel.updateEventListener(_ffi.sessionId, widget.id);
    }
    debugPrint("File manager page init success with id ${widget.id}");
    _ffi.dialogManager.setOverlayState(_overlayKeyState);
    // Call onSelected in post frame callback, since we cannot guarantee that the callback will not call setState.
    WidgetsBinding.instance.addPostFrameCallback((_) {
      widget.tabController?.onSelected?.call(widget.id);
    });
    WidgetsBinding.instance.addObserver(this);
  }

  @override
  void dispose() {
    model.close().whenComplete(() {
      _ffi.close();
      _ffi.dialogManager.dismissAll();
      if (!isLinux) {
        WakelockPlus.disable();
      }
      Get.delete<FFI>(tag: 'ft_${widget.id}');
    });
    WidgetsBinding.instance.removeObserver(this);
    super.dispose();
  }

  @override
  bool get wantKeepAlive => true;

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    super.didChangeAppLifecycleState(state);
    if (state == AppLifecycleState.resumed) {
      jobController.jobTable.refresh();
    }
  }

  Widget willPopScope(Widget child) {
    if (isWeb) {
      return WillPopScope(
        onWillPop: () async {
          clientClose(_ffi.sessionId, _ffi);
          return false;
        },
        child: child,
      );
    } else {
      return child;
    }
  }

  @override
  Widget build(BuildContext context) {
    super.build(context);
    return Overlay(key: _overlayKeyState.key, initialEntries: [
      OverlayEntry(builder: (_) {
        return willPopScope(Scaffold(
          backgroundColor: Theme.of(context).scaffoldBackgroundColor,
          body: Row(
            children: [
              if (!isWeb)
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
        ));
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
    Widget getIcon(JobProgress job) {
      final color = Theme.of(context).tabBarTheme.labelColor;
      switch (job.type) {
        case JobType.deleteDir:
        case JobType.deleteFile:
          return Icon(Icons.delete_outline, color: color);
        default:
          return Transform.rotate(
            angle: isWeb
                ? job.isRemoteToLocal
                    ? pi / 2
                    : pi / 2 * 3
                : job.isRemoteToLocal
                    ? pi
                    : 0,
            child: Icon(Icons.arrow_forward_ios, color: color),
          );
      }
    }

    statusListView(List<JobProgress> jobs) => ListView.builder(
          controller: ScrollController(),
          itemBuilder: (BuildContext context, int index) {
            final item = jobs[index];
            final status = item.getStatus();
            return Padding(
              padding: const EdgeInsets.only(bottom: 5),
              child: generateCard(
                Column(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Row(
                      crossAxisAlignment: CrossAxisAlignment.center,
                      children: [
                        getIcon(item)
                            .marginSymmetric(horizontal: 10, vertical: 12),
                        Expanded(
                          child: Column(
                            mainAxisSize: MainAxisSize.min,
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                              Tooltip(
                                waitDuration: Duration(milliseconds: 500),
                                message: item.jobName,
                                child: ExtendedText(
                                  item.jobName,
                                  maxLines: 1,
                                  overflow: TextOverflow.ellipsis,
                                  overflowWidget: TextOverflowWidget(
                                      child: Text("..."),
                                      position: TextOverflowPosition.start),
                                ),
                              ),
                              Tooltip(
                                waitDuration: Duration(milliseconds: 500),
                                message: status,
                                child: Text(status,
                                    style: TextStyle(
                                      fontSize: 12,
                                      color: MyTheme.darkGray,
                                    )).marginOnly(top: 6),
                              ),
                              Offstage(
                                offstage: item.type != JobType.transfer ||
                                    item.state != JobState.inProgress,
                                child: LinearPercentIndicator(
                                  animateFromLastPercent: true,
                                  center: Text(
                                    '${(item.finishedSize / item.totalSize * 100).toStringAsFixed(0)}%',
                                  ),
                                  barRadius: Radius.circular(15),
                                  percent: item.finishedSize / item.totalSize,
                                  progressColor: MyTheme.accent,
                                  backgroundColor: Theme.of(context).hoverColor,
                                  lineHeight: kDesktopFileTransferRowHeight,
                                ).paddingSymmetric(vertical: 8),
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
                                tooltip: translate("Resume"),
                                onPressed: () {
                                  jobController.resumeJob(item.id);
                                },
                                child: SvgPicture.asset(
                                  "assets/refresh.svg",
                                  colorFilter: svgColor(Colors.white),
                                ),
                                color: MyTheme.accent,
                                hoverColor: MyTheme.accent80,
                              ),
                            ),
                            MenuButton(
                              tooltip: translate("Delete"),
                              child: SvgPicture.asset(
                                "assets/close.svg",
                                colorFilter: svgColor(Colors.white),
                              ),
                              onPressed: () {
                                jobController.jobTable.removeAt(index);
                                jobController.cancelJob(item.id);
                              },
                              color: MyTheme.accent,
                              hoverColor: MyTheme.accent80,
                            ),
                          ],
                        ).marginAll(12),
                      ],
                    ),
                  ],
                ),
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
                            colorFilter: svgColor(
                                Theme.of(context).tabBarTheme.labelColor),
                            height: 40,
                          ).paddingOnly(bottom: 10),
                          Text(
                            translate("No transfers in progress"),
                            textAlign: TextAlign.center,
                            textScaler: TextScaler.linear(1.20),
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
  final _nameColWidth = 0.0.obs;
  final _modifiedColWidth = 0.0.obs;
  final _sizeColWidth = 0.0.obs;
  final _fileListScrollController = ScrollController();
  final _globalHeaderKey = GlobalKey();

  /// [_lastClickTime], [_lastClickEntry] help to handle double click
  var _lastClickTime =
      DateTime.now().millisecondsSinceEpoch - bind.getDoubleClickTime() - 1000;
  Entry? _lastClickEntry;

  double? _windowWidthPrev;
  double _fileTransferMinimumWidth = 0.0;

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
    _handleColumnPorportions();
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

  void _handleColumnPorportions() {
    final windowWidthNow = MediaQuery.of(context).size.width;
    if (_windowWidthPrev == null) {
      _windowWidthPrev = windowWidthNow;
      final defaultColumnWidth = windowWidthNow * 0.115;
      _fileTransferMinimumWidth = defaultColumnWidth / 3;
      _nameColWidth.value = defaultColumnWidth;
      _modifiedColWidth.value = defaultColumnWidth;
      _sizeColWidth.value = defaultColumnWidth;
    }

    if (_windowWidthPrev != windowWidthNow) {
      final difference = windowWidthNow / _windowWidthPrev!;
      _windowWidthPrev = windowWidthNow;
      _fileTransferMinimumWidth *= difference;
      _nameColWidth.value *= difference;
      _modifiedColWidth.value *= difference;
      _sizeColWidth.value *= difference;
    }
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
    var uploadButtonTapPosition = RelativeRect.fill;
    RxBool isUploadFolder =
        (bind.mainGetLocalOption(key: 'upload-folder-button') == 'Y').obs;
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
                                  sessionId: _ffi.sessionId,
                                  isRemote: !isLocal),
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
                    tooltip: translate('Back'),
                    padding: EdgeInsets.only(
                      right: 3,
                    ),
                    child: RotatedBox(
                      quarterTurns: 2,
                      child: SvgPicture.asset(
                        "assets/arrow.svg",
                        colorFilter:
                            svgColor(Theme.of(context).tabBarTheme.labelColor),
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
                    tooltip: translate('Parent directory'),
                    child: RotatedBox(
                      quarterTurns: 3,
                      child: SvgPicture.asset(
                        "assets/arrow.svg",
                        colorFilter:
                            svgColor(Theme.of(context).tabBarTheme.labelColor),
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
                      tooltip: translate('Search'),
                      onPressed: () {
                        _locationStatus.value = LocationStatus.fileSearchBar;
                        Future.delayed(
                            Duration.zero, () => _locationNode.requestFocus());
                      },
                      child: SvgPicture.asset(
                        "assets/search.svg",
                        colorFilter:
                            svgColor(Theme.of(context).tabBarTheme.labelColor),
                      ),
                      color: Theme.of(context).cardColor,
                      hoverColor: Theme.of(context).hoverColor,
                    );
                  case LocationStatus.pathLocation:
                    return MenuButton(
                      onPressed: null,
                      child: SvgPicture.asset(
                        "assets/close.svg",
                        colorFilter:
                            svgColor(Theme.of(context).tabBarTheme.labelColor),
                      ),
                      color: Theme.of(context).disabledColor,
                      hoverColor: Theme.of(context).hoverColor,
                    );
                  case LocationStatus.fileSearchBar:
                    return MenuButton(
                      tooltip: translate('Clear'),
                      onPressed: () {
                        onSearchText("", isLocal);
                        _locationStatus.value = LocationStatus.bread;
                      },
                      child: SvgPicture.asset(
                        "assets/close.svg",
                        colorFilter:
                            svgColor(Theme.of(context).tabBarTheme.labelColor),
                      ),
                      color: Theme.of(context).cardColor,
                      hoverColor: Theme.of(context).hoverColor,
                    );
                }
              }),
              MenuButton(
                tooltip: translate('Refresh File'),
                padding: EdgeInsets.only(
                  left: 3,
                ),
                onPressed: () {
                  controller.refresh();
                },
                child: SvgPicture.asset(
                  "assets/refresh.svg",
                  colorFilter:
                      svgColor(Theme.of(context).tabBarTheme.labelColor),
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
                      tooltip: translate('Home'),
                      padding: EdgeInsets.only(
                        right: 3,
                      ),
                      onPressed: () {
                        controller.goToHomeDirectory();
                      },
                      child: SvgPicture.asset(
                        "assets/home.svg",
                        colorFilter:
                            svgColor(Theme.of(context).tabBarTheme.labelColor),
                      ),
                      color: Theme.of(context).cardColor,
                      hoverColor: Theme.of(context).hoverColor,
                    ),
                    MenuButton(
                      tooltip: translate('Create Folder'),
                      onPressed: () {
                        final name = TextEditingController();
                        String? errorText;
                        _ffi.dialogManager.show((setState, close, context) {
                          name.addListener(() {
                            if (errorText != null) {
                              setState(() {
                                errorText = null;
                              });
                            }
                          });
                          submit() {
                            if (name.value.text.isNotEmpty) {
                              if (!PathUtil.validName(name.value.text,
                                  controller.options.value.isWindows)) {
                                setState(() {
                                  errorText = translate("Invalid folder name");
                                });
                                return;
                              }
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
                                    colorFilter: svgColor(MyTheme.accent)),
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
                                    errorText: errorText,
                                  ),
                                  controller: name,
                                  autofocus: true,
                                ).workaroundFreezeLinuxMint(),
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
                        colorFilter:
                            svgColor(Theme.of(context).tabBarTheme.labelColor),
                      ),
                      color: Theme.of(context).cardColor,
                      hoverColor: Theme.of(context).hoverColor,
                    ),
                    Obx(() => MenuButton(
                          tooltip: translate('Delete'),
                          onPressed: SelectedItems.valid(selectedItems.items)
                              ? () async {
                                  await (controller
                                      .removeAction(selectedItems));
                                  selectedItems.clear();
                                }
                              : null,
                          child: SvgPicture.asset(
                            "assets/trash.svg",
                            colorFilter: svgColor(
                                Theme.of(context).tabBarTheme.labelColor),
                          ),
                          color: Theme.of(context).cardColor,
                          hoverColor: Theme.of(context).hoverColor,
                        )),
                    menu(isLocal: isLocal),
                  ],
                ),
              ),
              if (isWeb)
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
                      onPressed: () =>
                          {webselectFiles(is_folder: isUploadFolder.value)},
                      label: InkWell(
                        hoverColor: Colors.transparent,
                        splashColor: Colors.transparent,
                        highlightColor: Colors.transparent,
                        focusColor: Colors.transparent,
                        onTapDown: (e) {
                          final x = e.globalPosition.dx;
                          final y = e.globalPosition.dy;
                          uploadButtonTapPosition =
                              RelativeRect.fromLTRB(x, y, x, y);
                        },
                        onTap: () async {
                          final value = await showMenu<bool>(
                              context: context,
                              position: uploadButtonTapPosition,
                              items: [
                                PopupMenuItem<bool>(
                                  value: false,
                                  child: Text(translate('Upload files')),
                                ),
                                PopupMenuItem<bool>(
                                  value: true,
                                  child: Text(translate('Upload folder')),
                                ),
                              ]);
                          if (value != null) {
                            isUploadFolder.value = value;
                            bind.mainSetLocalOption(
                                key: 'upload-folder-button',
                                value: value ? 'Y' : '');
                            webselectFiles(is_folder: value);
                          }
                        },
                        child: Icon(Icons.arrow_drop_down),
                      ),
                      icon: Text(
                        translate(isUploadFolder.isTrue
                            ? 'Upload folder'
                            : 'Upload files'),
                        textAlign: TextAlign.right,
                        style: TextStyle(
                          color: Colors.white,
                        ),
                      ).marginOnly(left: 8),
                    )).marginOnly(left: 16),
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
                        : isWeb
                            ? Offstage()
                            : RotatedBox(
                                quarterTurns: 2,
                                child: SvgPicture.asset(
                                  "assets/arrow.svg",
                                  colorFilter: svgColor(
                                      selectedItems.items.isEmpty
                                          ? Theme.of(context).brightness ==
                                                  Brightness.light
                                              ? MyTheme.grayBg
                                              : MyTheme.darkGray
                                          : Colors.white),
                                  alignment: Alignment.bottomRight,
                                ),
                              ),
                    label: isLocal
                        ? SvgPicture.asset(
                            "assets/arrow.svg",
                            colorFilter: svgColor(selectedItems.items.isEmpty
                                ? Theme.of(context).brightness ==
                                        Brightness.light
                                    ? MyTheme.grayBg
                                    : MyTheme.darkGray
                                : Colors.white),
                          )
                        : Text(
                            translate(isWeb ? 'Download' : 'Receive'),
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
          return controller.options.value.showHidden;
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
        tooltip: translate('More'),
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
          colorFilter: svgColor(Theme.of(context).tabBarTheme.labelColor),
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
    Rx<Entry?> rightClickEntry = Rx(null);

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
          var secondaryPosition = RelativeRect.fromLTRB(0, 0, 0, 0);
          onTap() {
            final items = selectedItems;
            // handle double click
            if (_checkDoubleClick(entry)) {
              controller.openDirectory(entry.path);
              items.clear();
              return;
            }
            _onSelectedChanged(items, filteredEntries, entry, isLocal);
          }

          onSecondaryTap() {
            final items = [
              if (!entry.isDrive &&
                  versionCmp(_ffi.ffiModel.pi.version, "1.3.0") >= 0)
                mod_menu.PopupMenuItem(
                  child: Text(translate("Rename")),
                  height: CustomPopupMenuTheme.height,
                  onTap: () {
                    controller.renameAction(entry, isLocal);
                  },
                )
            ];
            if (items.isNotEmpty) {
              rightClickEntry.value = entry;
              final future = mod_menu.showMenu(
                context: context,
                position: secondaryPosition,
                items: items,
              );
              future.then((value) {
                rightClickEntry.value = null;
              });
              future.onError((error, stackTrace) {
                rightClickEntry.value = null;
              });
            }
          }

          onSecondaryTapDown(details) {
            secondaryPosition = RelativeRect.fromLTRB(
                details.globalPosition.dx,
                details.globalPosition.dy,
                details.globalPosition.dx,
                details.globalPosition.dy);
          }

          return Padding(
            padding: EdgeInsets.symmetric(vertical: 1),
            child: Obx(() => Container(
                decoration: BoxDecoration(
                  color: selectedItems.items.contains(entry)
                      ? MyTheme.button
                      : Theme.of(context).cardColor,
                  borderRadius: BorderRadius.all(
                    Radius.circular(5.0),
                  ),
                  border: rightClickEntry.value == entry
                      ? Border.all(
                          color: MyTheme.button,
                          width: 1.0,
                        )
                      : null,
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
                                                colorFilter: svgColor(
                                                    Theme.of(context)
                                                        .tabBarTheme
                                                        .labelColor),
                                              ),
                                        Expanded(
                                            child: Text(entry.name.nonBreaking,
                                                style: TextStyle(
                                                    color: selectedItems.items
                                                            .contains(entry)
                                                        ? Colors.white
                                                        : null),
                                                overflow:
                                                    TextOverflow.ellipsis))
                                      ]),
                                    )),
                              ),
                              onTap: onTap,
                              onSecondaryTap: onSecondaryTap,
                              onSecondaryTapDown: onSecondaryTapDown,
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
                                          color: selectedItems.items
                                                  .contains(entry)
                                              ? Colors.white70
                                              : MyTheme.darkGray,
                                        ),
                                      )),
                                ),
                              ),
                              onTap: onTap,
                              onSecondaryTap: onSecondaryTap,
                              onSecondaryTapDown: onSecondaryTapDown,
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
                                        fontSize: 10,
                                        color:
                                            selectedItems.items.contains(entry)
                                                ? Colors.white70
                                                : MyTheme.darkGray),
                                  ),
                                ),
                                onTap: onTap,
                                onSecondaryTap: onSecondaryTap,
                                onSecondaryTapDown: onSecondaryTapDown,
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
            .contains(LogicalKeyboardKey.controlLeft) ||
        RawKeyboard.instance.keysPressed
            .contains(LogicalKeyboardKey.controlRight);
    final isShiftDown = RawKeyboard.instance.keysPressed
            .contains(LogicalKeyboardKey.shiftLeft) ||
        RawKeyboard.instance.keysPressed
            .contains(LogicalKeyboardKey.shiftRight);
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

  void _onDrag(double dx, RxDouble column1, RxDouble column2) {
    if (column1.value + dx <= _fileTransferMinimumWidth ||
        column2.value - dx <= _fileTransferMinimumWidth) {
      return;
    }
    column1.value += dx;
    column2.value -= dx;
    column1.value = max(_fileTransferMinimumWidth, column1.value);
    column2.value = max(_fileTransferMinimumWidth, column2.value);
  }

  Widget _buildFileBrowserHeader(BuildContext context) {
    final padding = EdgeInsets.all(1.0);
    return SizedBox(
      key: _globalHeaderKey,
      height: kDesktopFileTransferHeaderHeight,
      child: Row(
        children: [
          Obx(
            () => headerItemFunc(
                _nameColWidth.value, SortBy.name, translate("Name")),
          ),
          DraggableDivider(
            axis: Axis.vertical,
            onPointerMove: (dx) =>
                _onDrag(dx, _nameColWidth, _modifiedColWidth),
            padding: padding,
          ),
          Obx(
            () => headerItemFunc(_modifiedColWidth.value, SortBy.modified,
                translate("Modified")),
          ),
          DraggableDivider(
              axis: Axis.vertical,
              onPointerMove: (dx) =>
                  _onDrag(dx, _modifiedColWidth, _sizeColWidth),
              padding: padding),
          Expanded(
              child: headerItemFunc(
                  _sizeColWidth.value, SortBy.size, translate("Size")))
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
                    Expanded(
                      child: Text(
                        name,
                        style: headerTextStyle,
                        overflow: TextOverflow.ellipsis,
                      ).marginOnly(left: 4),
                    ),
                    ascending.value != null
                        ? Icon(
                            ascending.value!
                                ? Icons.keyboard_arrow_up_rounded
                                : Icons.keyboard_arrow_down_rounded,
                          )
                        : SizedBox()
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
                        final scale = isWindows ? 2 : 4;
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
          colorFilter: svgColor(Theme.of(context).tabBarTheme.labelColor),
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
          ).workaroundFreezeLinuxMint(),
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
