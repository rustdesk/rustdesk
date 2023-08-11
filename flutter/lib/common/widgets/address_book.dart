import 'package:flutter/material.dart';
import 'package:flutter_hbb/common/formatter/id_formatter.dart';
import 'package:flutter_hbb/common/widgets/peer_card.dart';
import 'package:flutter_hbb/common/widgets/peers_view.dart';
import 'package:flutter_hbb/desktop/widgets/popup_menu.dart';
import 'package:flutter_hbb/models/ab_model.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import '../../desktop/widgets/material_mod_popup_menu.dart' as mod_menu;
import 'package:get/get.dart';

import '../../common.dart';
import 'dialog.dart';
import 'loading_dot_widget.dart';
import 'login.dart';

final hideAbTagsPanel = false.obs;

class AddressBook extends StatefulWidget {
  final EdgeInsets? menuPadding;
  const AddressBook({Key? key, this.menuPadding}) : super(key: key);

  @override
  State<StatefulWidget> createState() {
    return _AddressBookState();
  }
}

class _AddressBookState extends State<AddressBook> {
  var menuPos = RelativeRect.fill;

  @override
  void initState() {
    super.initState();
  }

  @override
  Widget build(BuildContext context) => Obx(() {
        if (gFFI.userModel.userName.value.isEmpty) {
          return Center(
              child: ElevatedButton(
                  onPressed: loginDialog, child: Text(translate("Login"))));
        } else {
          if (gFFI.abModel.abLoading.value && gFFI.abModel.emtpy) {
            return const Center(
              child: CircularProgressIndicator(),
            );
          }
          if (gFFI.abModel.abError.isNotEmpty) {
            return _buildShowError(gFFI.abModel.abError.value);
          }
          return Column(
            children: [
              _buildLoadingHavingPeers(),
              Expanded(
                  child: isDesktop
                      ? _buildAddressBookDesktop()
                      : _buildAddressBookMobile())
            ],
          );
        }
      });

  Widget _buildShowError(String error) {
    return Center(
        child: Column(
      mainAxisAlignment: MainAxisAlignment.center,
      children: [
        Text(translate(error)),
        TextButton(
            onPressed: () {
              gFFI.abModel.pullAb();
            },
            child: Text(translate("Retry")))
      ],
    ));
  }

  Widget _buildLoadingHavingPeers() {
    double size = 15;
    return Obx(() => Offstage(
          offstage: !(gFFI.abModel.abLoading.value && !gFFI.abModel.emtpy),
          child: Row(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              SizedBox(
                      height: size,
                      child: Center(child: LoadingDotWidget(size: size)))
                  .marginSymmetric(vertical: 10)
            ],
          ),
        ));
  }

  Widget _buildAddressBookDesktop() {
    return Row(
      children: [
        Offstage(
            offstage: hideAbTagsPanel.value,
            child: Container(
              decoration: BoxDecoration(
                  borderRadius: BorderRadius.circular(12),
                  border: Border.all(
                      color: Theme.of(context).colorScheme.background)),
              child: Container(
                width: 150,
                height: double.infinity,
                padding: const EdgeInsets.all(8.0),
                child: Column(
                  children: [
                    _buildTagHeader().marginOnly(left: 8.0, right: 0),
                    Expanded(
                      child: Container(
                        width: double.infinity,
                        height: double.infinity,
                        child: _buildTags(),
                      ),
                    )
                  ],
                ),
              ),
            ).marginOnly(right: 12.0)),
        _buildPeersViews()
      ],
    );
  }

  Widget _buildAddressBookMobile() {
    return Column(
      children: [
        Offstage(
            offstage: hideAbTagsPanel.value,
            child: Container(
              decoration: BoxDecoration(
                  borderRadius: BorderRadius.circular(6),
                  border: Border.all(
                      color: Theme.of(context).colorScheme.background)),
              child: Container(
                padding: const EdgeInsets.all(8.0),
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    _buildTagHeader().marginOnly(left: 8.0, right: 0),
                    Container(
                      width: double.infinity,
                      child: _buildTags(),
                    ),
                  ],
                ),
              ),
            ).marginOnly(bottom: 12.0)),
        _buildPeersViews()
      ],
    );
  }

  Widget _buildTagHeader() {
    return Row(
      mainAxisAlignment: MainAxisAlignment.spaceBetween,
      children: [
        Text(translate('Tags')),
        Listener(
            onPointerDown: (e) {
              final x = e.position.dx;
              final y = e.position.dy;
              menuPos = RelativeRect.fromLTRB(x, y, x, y);
            },
            onPointerUp: (_) => _showMenu(menuPos),
            child: build_more(context, invert: true)),
      ],
    );
  }

  Widget _buildTags() {
    return Obx(() {
      final List tags;
      if (gFFI.abModel.sortTags.value) {
        tags = gFFI.abModel.tags.toList();
        tags.sort();
      } else {
        tags = gFFI.abModel.tags;
      }
      return Wrap(
        children: tags
            .map((e) => AddressBookTag(
                name: e,
                tags: gFFI.abModel.selectedTags,
                onTap: () {
                  if (gFFI.abModel.selectedTags.contains(e)) {
                    gFFI.abModel.selectedTags.remove(e);
                  } else {
                    gFFI.abModel.selectedTags.add(e);
                  }
                }))
            .toList(),
      );
    });
  }

  Widget _buildPeersViews() {
    return Expanded(
      child: Align(
          alignment: Alignment.topLeft,
          child: Obx(() => AddressBookPeersView(
                menuPadding: widget.menuPadding,
                // ignore: invalid_use_of_protected_member
                initPeers: gFFI.abModel.peers.value,
              ))),
    );
  }

  @protected
  MenuEntryBase<String> syncMenuItem() {
    return MenuEntrySwitch<String>(
      switchType: SwitchType.scheckbox,
      text: translate('Sync with recent sessions'),
      getter: () async {
        return shouldSyncAb();
      },
      setter: (bool v) async {
        bind.mainSetLocalOption(key: syncAbOption, value: v ? 'Y' : '');
      },
      dismissOnClicked: true,
    );
  }

  @protected
  MenuEntryBase<String> sortMenuItem() {
    return MenuEntrySwitch<String>(
      switchType: SwitchType.scheckbox,
      text: translate('Sort tags'),
      getter: () async {
        return shouldSortTags();
      },
      setter: (bool v) async {
        bind.mainSetLocalOption(key: sortAbTagsOption, value: v ? 'Y' : '');
        gFFI.abModel.sortTags.value = v;
      },
      dismissOnClicked: true,
    );
  }

  void _showMenu(RelativeRect pos) {
    final items = [
      getEntry(translate("Add ID"), abAddId),
      getEntry(translate("Add Tag"), abAddTag),
      getEntry(translate("Unselect all tags"), gFFI.abModel.unsetSelectedTags),
      sortMenuItem(),
      syncMenuItem(),
    ];

    mod_menu.showMenu(
      context: context,
      position: pos,
      items: items
          .map((e) => e.build(
              context,
              MenuConfig(
                  commonColor: CustomPopupMenuTheme.commonColor,
                  height: CustomPopupMenuTheme.height,
                  dividerHeight: CustomPopupMenuTheme.dividerHeight)))
          .expand((i) => i)
          .toList(),
      elevation: 8,
    );
  }

  void abAddId() async {
    if (gFFI.abModel.isFull(true)) {
      return;
    }
    var isInProgress = false;
    IDTextEditingController idController = IDTextEditingController(text: '');
    TextEditingController aliasController = TextEditingController(text: '');
    final tags = List.of(gFFI.abModel.tags);
    var selectedTag = List<dynamic>.empty(growable: true).obs;
    final style = TextStyle(fontSize: 14.0);
    String? errorMsg;

    gFFI.dialogManager.show((setState, close, context) {
      submit() async {
        setState(() {
          isInProgress = true;
          errorMsg = null;
        });
        String id = idController.id;
        if (id.isEmpty) {
          // pass
        } else {
          if (gFFI.abModel.idContainBy(id)) {
            setState(() {
              isInProgress = false;
              errorMsg = translate('ID already exists');
            });
            return;
          }
          gFFI.abModel.addId(id, aliasController.text.trim(), selectedTag);
          await gFFI.abModel.pushAb();
          this.setState(() {});
          // final currentPeers
        }
        close();
      }

      return CustomAlertDialog(
        title: Text(translate("Add ID")),
        content: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Column(
              children: [
                Align(
                  alignment: Alignment.centerLeft,
                  child: Row(
                    children: [
                      Text(
                        '*',
                        style: TextStyle(color: Colors.red, fontSize: 14),
                      ),
                      Text(
                        'ID',
                        style: style,
                      ),
                    ],
                  ),
                ),
                TextField(
                  controller: idController,
                  inputFormatters: [IDTextInputFormatter()],
                  decoration: InputDecoration(errorText: errorMsg),
                ),
                Align(
                  alignment: Alignment.centerLeft,
                  child: Text(
                    translate('Alias'),
                    style: style,
                  ),
                ).marginOnly(top: 8, bottom: 2),
                TextField(
                  controller: aliasController,
                ),
                Align(
                  alignment: Alignment.centerLeft,
                  child: Text(
                    translate('Tags'),
                    style: style,
                  ),
                ).marginOnly(top: 8),
                Container(
                  child: Wrap(
                    children: tags
                        .map((e) => AddressBookTag(
                            name: e,
                            tags: selectedTag,
                            onTap: () {
                              if (selectedTag.contains(e)) {
                                selectedTag.remove(e);
                              } else {
                                selectedTag.add(e);
                              }
                            },
                            showActionMenu: false))
                        .toList(growable: false),
                  ),
                ),
              ],
            ),
            const SizedBox(
              height: 4.0,
            ),
            Offstage(
                offstage: !isInProgress, child: const LinearProgressIndicator())
          ],
        ),
        actions: [
          dialogButton("Cancel", onPressed: close, isOutline: true),
          dialogButton("OK", onPressed: submit),
        ],
        onSubmit: submit,
        onCancel: close,
      );
    });
  }

  void abAddTag() async {
    var field = "";
    var msg = "";
    var isInProgress = false;
    TextEditingController controller = TextEditingController(text: field);
    gFFI.dialogManager.show((setState, close, context) {
      submit() async {
        setState(() {
          msg = "";
          isInProgress = true;
        });
        field = controller.text.trim();
        if (field.isEmpty) {
          // pass
        } else {
          final tags = field.trim().split(RegExp(r"[\s,;\n]+"));
          field = tags.join(',');
          for (final tag in tags) {
            gFFI.abModel.addTag(tag);
          }
          await gFFI.abModel.pushAb();
          // final currentPeers
        }
        close();
      }

      return CustomAlertDialog(
        title: Text(translate("Add Tag")),
        content: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(translate("whitelist_sep")),
            const SizedBox(
              height: 8.0,
            ),
            Row(
              children: [
                Expanded(
                  child: TextField(
                    maxLines: null,
                    decoration: InputDecoration(
                      errorText: msg.isEmpty ? null : translate(msg),
                    ),
                    controller: controller,
                    autofocus: true,
                  ),
                ),
              ],
            ),
            const SizedBox(
              height: 4.0,
            ),
            Offstage(
                offstage: !isInProgress, child: const LinearProgressIndicator())
          ],
        ),
        actions: [
          dialogButton("Cancel", onPressed: close, isOutline: true),
          dialogButton("OK", onPressed: submit),
        ],
        onSubmit: submit,
        onCancel: close,
      );
    });
  }
}

class AddressBookTag extends StatelessWidget {
  final String name;
  final RxList<dynamic> tags;
  final Function()? onTap;
  final bool showActionMenu;

  const AddressBookTag(
      {Key? key,
      required this.name,
      required this.tags,
      this.onTap,
      this.showActionMenu = true})
      : super(key: key);

  @override
  Widget build(BuildContext context) {
    var pos = RelativeRect.fill;

    void setPosition(TapDownDetails e) {
      final x = e.globalPosition.dx;
      final y = e.globalPosition.dy;
      pos = RelativeRect.fromLTRB(x, y, x, y);
    }

    return GestureDetector(
      onTap: onTap,
      onTapDown: showActionMenu ? setPosition : null,
      onSecondaryTapDown: showActionMenu ? setPosition : null,
      onSecondaryTap: showActionMenu ? () => _showMenu(context, pos) : null,
      onLongPress: showActionMenu ? () => _showMenu(context, pos) : null,
      child: Obx(
        () => Container(
          decoration: BoxDecoration(
              color: tags.contains(name)
                  ? Colors.blue
                  : Theme.of(context).colorScheme.background,
              borderRadius: BorderRadius.circular(6)),
          margin: const EdgeInsets.symmetric(horizontal: 4.0, vertical: 8.0),
          padding: const EdgeInsets.symmetric(vertical: 2.0, horizontal: 8.0),
          child: Text(name,
              style:
                  TextStyle(color: tags.contains(name) ? Colors.white : null)),
        ),
      ),
    );
  }

  void _showMenu(BuildContext context, RelativeRect pos) {
    final items = [
      getEntry(translate("Rename"), () {
        renameDialog(
            oldName: name,
            validator: (String? newName) {
              if (newName == null || newName.isEmpty) {
                return translate('Can not be empty');
              }
              if (newName != name && gFFI.abModel.tags.contains(newName)) {
                return translate('Already exists');
              }
              return null;
            },
            onSubmit: (String newName) {
              if (name != newName) {
                gFFI.abModel.renameTag(name, newName);
                gFFI.abModel.pushAb();
              }
              Future.delayed(Duration.zero, () => Get.back());
            },
            onCancel: () {
              Future.delayed(Duration.zero, () => Get.back());
            });
      }),
      getEntry(translate("Delete"), () {
        gFFI.abModel.deleteTag(name);
        gFFI.abModel.pushAb();
        Future.delayed(Duration.zero, () => Get.back());
      }),
    ];

    mod_menu.showMenu(
      context: context,
      position: pos,
      items: items
          .map((e) => e.build(
              context,
              MenuConfig(
                  commonColor: CustomPopupMenuTheme.commonColor,
                  height: CustomPopupMenuTheme.height,
                  dividerHeight: CustomPopupMenuTheme.dividerHeight)))
          .expand((i) => i)
          .toList(),
      elevation: 8,
    );
  }
}

MenuEntryButton<String> getEntry(String title, VoidCallback proc) {
  return MenuEntryButton<String>(
    childBuilder: (TextStyle? style) => Text(
      title,
      style: style,
    ),
    proc: proc,
    dismissOnClicked: true,
  );
}
