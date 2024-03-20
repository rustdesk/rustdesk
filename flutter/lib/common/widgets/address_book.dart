import 'dart:math';

import 'package:bot_toast/bot_toast.dart';
import 'package:dropdown_button2/dropdown_button2.dart';
import 'package:dynamic_layouts/dynamic_layouts.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common/formatter/id_formatter.dart';
import 'package:flutter_hbb/common/hbbs/hbbs.dart';
import 'package:flutter_hbb/common/widgets/peer_card.dart';
import 'package:flutter_hbb/common/widgets/peers_view.dart';
import 'package:flutter_hbb/desktop/widgets/popup_menu.dart';
import 'package:flutter_hbb/models/ab_model.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_simple_treeview/flutter_simple_treeview.dart';
import '../../desktop/widgets/material_mod_popup_menu.dart' as mod_menu;
import 'package:get/get.dart';
import 'package:flex_color_picker/flex_color_picker.dart';

import '../../common.dart';
import 'dialog.dart';
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
        if (!gFFI.userModel.isLogin) {
          return Center(
              child: ElevatedButton(
                  onPressed: loginDialog, child: Text(translate("Login"))));
        } else {
          if (gFFI.abModel.currentAbLoading.value &&
              gFFI.abModel.currentAbEmtpy) {
            return const Center(
              child: CircularProgressIndicator(),
            );
          }
          return Column(
            children: [
              buildErrorBanner(context,
                  loading: gFFI.abModel.currentAbLoading,
                  err: gFFI.abModel.currentAbPullError,
                  retry: null,
                  close: () => gFFI.abModel.currentAbPullError.value = ''),
              buildErrorBanner(context,
                  loading: gFFI.abModel.currentAbLoading,
                  err: gFFI.abModel.currentAbPushError,
                  retry: null, // remove retry
                  close: () => gFFI.abModel.currentAbPushError.value = ''),
              Expanded(
                  child: isDesktop
                      ? _buildAddressBookDesktop()
                      : _buildAddressBookMobile())
            ],
          );
        }
      });

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
                width: 180,
                height: double.infinity,
                padding: const EdgeInsets.all(8.0),
                child: Column(
                  children: [
                    _buildAbDropdown(),
                    _buildTagHeader().marginOnly(left: 8.0, right: 0),
                    Expanded(
                      child: Container(
                        width: double.infinity,
                        height: double.infinity,
                        child: _buildTags(),
                      ),
                    ),
                    _buildAbPermission(),
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
                    _buildAbDropdown(),
                    _buildTagHeader().marginOnly(left: 8.0, right: 0),
                    Container(
                      width: double.infinity,
                      child: _buildTags(),
                    ),
                    _buildAbPermission(),
                  ],
                ),
              ),
            ).marginOnly(bottom: 12.0)),
        _buildPeersViews()
      ],
    );
  }

  Widget _buildAbPermission() {
    icon(IconData data, String tooltip) {
      return Tooltip(
          message: translate(tooltip),
          waitDuration: Duration.zero,
          child: Icon(data, size: 12.0).marginSymmetric(horizontal: 2.0));
    }

    return Obx(() {
      if (gFFI.abModel.legacyMode.value) return Offstage();
      if (gFFI.abModel.current.isPersonal()) {
        return Row(
          mainAxisAlignment: MainAxisAlignment.end,
          children: [
            icon(Icons.cloud_off, "Personal"),
          ],
        );
      } else {
        List<Widget> children = [];
        final rule = gFFI.abModel.current.sharedProfile()?.rule;
        if (rule == ShareRule.read.value) {
          children.add(
              icon(Icons.visibility, ShareRule.desc(ShareRule.read.value)));
        } else if (rule == ShareRule.readWrite.value) {
          children
              .add(icon(Icons.edit, ShareRule.desc(ShareRule.readWrite.value)));
        } else if (rule == ShareRule.fullControl.value) {
          children.add(icon(
              Icons.security, ShareRule.desc(ShareRule.fullControl.value)));
        }
        final owner = gFFI.abModel.current.sharedProfile()?.owner;
        if (owner != null) {
          children.add(icon(Icons.person, "${translate("Owner")}: $owner"));
        }
        return Row(
          mainAxisAlignment: MainAxisAlignment.end,
          children: children,
        );
      }
    });
  }

  Widget _buildAbDropdown() {
    if (gFFI.abModel.legacyMode.value) {
      return Offstage();
    }
    final names = gFFI.abModel.addressBookNames();
    if (!names.contains(gFFI.abModel.currentName.value)) {
      return Offstage();
    }
    final TextEditingController textEditingController = TextEditingController();

    return DropdownButton2<String>(
      value: gFFI.abModel.currentName.value,
      onChanged: (value) {
        if (value != null) {
          gFFI.abModel.setCurrentName(value);
          bind.setLocalFlutterOption(k: 'current-ab-name', v: value);
        }
      },
      items: names
          .map((e) => DropdownMenuItem(
              value: e,
              child: Row(
                children: [
                  Expanded(
                    child: Tooltip(
                        message: e,
                        child: Text(gFFI.abModel.translatedName(e),
                            style: TextStyle(fontSize: 14))),
                  ),
                ],
              )))
          .toList(),
      isExpanded: true,
      dropdownSearchData: DropdownSearchData(
        searchController: textEditingController,
        searchInnerWidgetHeight: 50,
        searchInnerWidget: Container(
          height: 50,
          padding: const EdgeInsets.only(
            top: 8,
            bottom: 4,
            right: 8,
            left: 8,
          ),
          child: TextFormField(
            expands: true,
            maxLines: null,
            controller: textEditingController,
            decoration: InputDecoration(
              isDense: true,
              contentPadding: const EdgeInsets.symmetric(
                horizontal: 10,
                vertical: 8,
              ),
              hintText: translate('Search'),
              hintStyle: const TextStyle(fontSize: 12),
              border: OutlineInputBorder(
                borderRadius: BorderRadius.circular(8),
              ),
            ),
          ),
        ),
        searchMatchFn: (item, searchValue) {
          return item.value
              .toString()
              .toLowerCase()
              .contains(searchValue.toLowerCase());
        },
      ),
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
        tags = gFFI.abModel.currentAbTags.toList();
        tags.sort();
      } else {
        tags = gFFI.abModel.currentAbTags;
      }
      final editPermission = gFFI.abModel.current.canWrite();
      tagBuilder(String e) {
        return AddressBookTag(
            name: e,
            tags: gFFI.abModel.selectedTags,
            onTap: () {
              if (gFFI.abModel.selectedTags.contains(e)) {
                gFFI.abModel.selectedTags.remove(e);
              } else {
                gFFI.abModel.selectedTags.add(e);
              }
            },
            showActionMenu: editPermission);
      }

      final gridView = DynamicGridView.builder(
          shrinkWrap: isMobile,
          gridDelegate: SliverGridDelegateWithWrapping(),
          itemCount: tags.length,
          itemBuilder: (BuildContext context, int index) {
            final e = tags[index];
            return tagBuilder(e);
          });
      final maxHeight = max(MediaQuery.of(context).size.height / 6, 100.0);
      return isDesktop
          ? gridView
          : LimitedBox(maxHeight: maxHeight, child: gridView);
    });
  }

  Widget _buildPeersViews() {
    return Expanded(
      child: Align(
          alignment: Alignment.topLeft,
          child: AddressBookPeersView(
            menuPadding: widget.menuPadding,
            getInitPeers: () => gFFI.abModel.currentAbPeers,
          )),
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
        gFFI.abModel.setShouldAsync(v);
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

  @protected
  MenuEntryBase<String> filterMenuItem() {
    return MenuEntrySwitch<String>(
      switchType: SwitchType.scheckbox,
      text: translate('Filter by intersection'),
      getter: () async {
        return filterAbTagByIntersection();
      },
      setter: (bool v) async {
        bind.mainSetLocalOption(key: filterAbTagOption, value: v ? 'Y' : '');
        gFFI.abModel.filterByIntersection.value = v;
      },
      dismissOnClicked: true,
    );
  }

  void _showMenu(RelativeRect pos) {
    final currentProfile = gFFI.abModel.current.sharedProfile();
    final shardFullControl = !gFFI.abModel.current.isPersonal() &&
        gFFI.abModel.current.fullControl();
    final shared = [
      getEntry(translate('Add shared address book'),
          () => createOrUpdateSharedAb(null)),
      if (gFFI.abModel.current.fullControl() &&
          !gFFI.abModel.current.isPersonal())
        getEntry(translate('Update this address book'),
            () => createOrUpdateSharedAb(currentProfile)),
      if (shardFullControl)
        getEntry(translate('Delete this address book'), deleteSharedAb),
      if (shardFullControl)
        getEntry(translate('Share this address book'), shareAb),
      MenuEntryDivider<String>(),
    ];
    final canWrite = gFFI.abModel.current.canWrite();
    final items = [
      if (!gFFI.abModel.legacyMode.value) ...shared,
      if (canWrite) getEntry(translate("Add ID"), addIdToCurrentAb),
      if (canWrite) getEntry(translate("Add Tag"), abAddTag),
      getEntry(translate("Unselect all tags"), gFFI.abModel.unsetSelectedTags),
      sortMenuItem(),
      syncMenuItem(),
      filterMenuItem(),
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

  void addIdToCurrentAb() async {
    if (gFFI.abModel.isCurrentAbFull(true)) {
      return;
    }
    var isInProgress = false;
    IDTextEditingController idController = IDTextEditingController(text: '');
    TextEditingController aliasController = TextEditingController(text: '');
    TextEditingController passwordController = TextEditingController(text: '');
    final tags = List.of(gFFI.abModel.currentAbTags);
    var selectedTag = List<dynamic>.empty(growable: true).obs;
    final style = TextStyle(fontSize: 14.0);
    String? errorMsg;
    final isCurrentAbShared = !gFFI.abModel.current.isPersonal();

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
          if (gFFI.abModel.idContainByCurrent(id)) {
            setState(() {
              isInProgress = false;
              errorMsg = translate('ID already exists');
            });
            return;
          }
          var password = '';
          if (isCurrentAbShared) {
            password = passwordController.text;
          }
          String? errMsg2 = await gFFI.abModel.addIdToCurrent(
              id, aliasController.text.trim(), password, selectedTag);
          if (errMsg2 != null) {
            setState(() {
              isInProgress = false;
              errorMsg = errMsg2;
            });
            return;
          }
          // final currentPeers
        }
        close();
      }

      double marginBottom = 4;
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
                ).marginOnly(bottom: marginBottom),
                TextField(
                  controller: idController,
                  inputFormatters: [IDTextInputFormatter()],
                  decoration:
                      InputDecoration(errorText: errorMsg, errorMaxLines: 5),
                ),
                Align(
                  alignment: Alignment.centerLeft,
                  child: Text(
                    translate('Alias'),
                    style: style,
                  ),
                ).marginOnly(top: 8, bottom: marginBottom),
                TextField(
                  controller: aliasController,
                ),
                if (isCurrentAbShared)
                  Align(
                    alignment: Alignment.centerLeft,
                    child: Text(
                      translate('Password'),
                      style: style,
                    ),
                  ).marginOnly(top: 8, bottom: marginBottom),
                if (isCurrentAbShared)
                  TextField(
                    controller: passwordController,
                    obscureText: true,
                  ),
                Align(
                  alignment: Alignment.centerLeft,
                  child: Text(
                    translate('Tags'),
                    style: style,
                  ),
                ).marginOnly(top: 8, bottom: marginBottom),
                Align(
                  alignment: Alignment.centerLeft,
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
            if (!gFFI.abModel.current.isPersonal())
              Row(children: [
                Icon(Icons.info, color: Colors.amber).marginOnly(right: 4),
                Text(
                  translate('share_warning_tip'),
                  style: TextStyle(fontSize: 12),
                )
              ]).marginSymmetric(vertical: 10),
            // NOT use Offstage to wrap LinearProgressIndicator
            if (isInProgress) const LinearProgressIndicator(),
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
          gFFI.abModel.addTags(tags);
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
            // NOT use Offstage to wrap LinearProgressIndicator
            if (isInProgress) const LinearProgressIndicator(),
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

  void createOrUpdateSharedAb(AbProfile? profile) async {
    final isAdd = profile == null;
    var msg = "";
    var isInProgress = false;
    final style = TextStyle(fontSize: 14.0);
    double marginBottom = 4;
    TextEditingController nameController =
        TextEditingController(text: profile?.name ?? '');
    TextEditingController noteController =
        TextEditingController(text: profile?.note ?? '');

    gFFI.dialogManager.show((setState, close, context) {
      submit() async {
        final name = nameController.text.trim();
        if (isAdd && name.isEmpty) {
          // pass
        } else {
          final note = noteController.text.trim();
          setState(() {
            msg = "";
            isInProgress = true;
          });
          final oldName = profile?.name;
          final errMsg = (profile == null
              ? await gFFI.abModel.addSharedAb(name, note)
              : await gFFI.abModel.updateSharedAb(profile.guid, name, note));
          if (errMsg.isNotEmpty) {
            setState(() {
              msg = errMsg;
              isInProgress = false;
            });
            return;
          }
          await gFFI.abModel.pullAb();
          if (gFFI.abModel.addressBookNames().contains(name)) {
            gFFI.abModel.setCurrentName(name);
          }
          // workaround for showing empty peers
          if (oldName != null && oldName != name) {
            Future.delayed(Duration.zero, () async {
              await gFFI.abModel.pullAb();
            });
          }
        }
        close();
      }

      return CustomAlertDialog(
        title: Text(translate(isAdd ? 'Add shared address book' : 'Update')),
        content: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
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
                    translate('Name'),
                    style: style,
                  ),
                ],
              ),
            ).marginOnly(bottom: marginBottom),
            Row(
              children: [
                Expanded(
                  child: TextField(
                    maxLines: null,
                    decoration: InputDecoration(
                      errorText: msg.isEmpty ? null : translate(msg),
                      errorMaxLines: 3,
                    ),
                    controller: nameController,
                    autofocus: true,
                  ),
                ),
              ],
            ),
            const SizedBox(
              height: 4.0,
            ),
            Align(
              alignment: Alignment.centerLeft,
              child: Text(
                translate('Note'),
                style: style,
              ),
            ).marginOnly(top: 8, bottom: marginBottom),
            TextField(
              controller: noteController,
              maxLength: 100,
            ),
            const SizedBox(
              height: 4.0,
            ),
            // NOT use Offstage to wrap LinearProgressIndicator
            if (isInProgress) const LinearProgressIndicator(),
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

  void deleteSharedAb() async {
    RxBool isInProgress = false.obs;

    String currentName = gFFI.abModel.currentName.value;
    gFFI.dialogManager.show((setState, close, context) {
      submit() async {
        isInProgress.value = true;
        String errMsg = await gFFI.abModel.deleteSharedAb(currentName);
        close();
        isInProgress.value = false;
        if (errMsg.isEmpty) {
          showToast(translate('Successful'));
        } else {
          BotToast.showText(contentColor: Colors.red, text: translate(errMsg));
        }
        gFFI.abModel.pullAb();
      }

      cancel() {
        close();
      }

      return CustomAlertDialog(
        content: Obx(() => Column(
              crossAxisAlignment: CrossAxisAlignment.center,
              children: [
                Text(translate(
                    'Are you sure you want to delete address book {$currentName}?')),
                // NOT use Offstage to wrap LinearProgressIndicator
                isInProgress.value
                    ? const LinearProgressIndicator()
                    : Offstage()
              ],
            )),
        actions: [
          dialogButton(
            "Cancel",
            icon: Icon(Icons.close_rounded),
            onPressed: cancel,
            isOutline: true,
          ),
          dialogButton(
            "OK",
            icon: Icon(Icons.done_rounded),
            onPressed: submit,
          ),
        ],
        onSubmit: submit,
        onCancel: cancel,
      );
    });
  }

  void shareAb() async {
    gFFI.dialogManager.show((setState, close, context) {
      return CustomAlertDialog(
        content: _RuleTree(),
        actions: [
          Row(children: [
            Icon(Icons.info, color: MyTheme.accent, size: 20)
                .marginSymmetric(horizontal: isDesktop ? 10 : 5),
            Expanded(
              child: Text(
                translate('permission_priority_tip'),
                style: TextStyle(fontSize: 12),
                textAlign: TextAlign.left,
              ),
            )
          ]),
          dialogButton(
            "Close",
            icon: Icon(Icons.close_rounded),
            onPressed: close,
            isOutline: true,
          ),
        ],
        onCancel: close,
        onSubmit: close,
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

    const double radius = 8;
    return GestureDetector(
      onTap: onTap,
      onTapDown: showActionMenu ? setPosition : null,
      onSecondaryTapDown: showActionMenu ? setPosition : null,
      onSecondaryTap: showActionMenu ? () => _showMenu(context, pos) : null,
      onLongPress: showActionMenu ? () => _showMenu(context, pos) : null,
      child: Obx(() => Container(
            decoration: BoxDecoration(
                color: tags.contains(name)
                    ? gFFI.abModel.getCurrentAbTagColor(name)
                    : Theme.of(context).colorScheme.background,
                borderRadius: BorderRadius.circular(4)),
            margin: const EdgeInsets.symmetric(horizontal: 4.0, vertical: 4.0),
            padding: const EdgeInsets.symmetric(vertical: 2.0, horizontal: 6.0),
            child: IntrinsicWidth(
              child: Row(
                children: [
                  Container(
                    width: radius,
                    height: radius,
                    decoration: BoxDecoration(
                        shape: BoxShape.circle,
                        color: tags.contains(name)
                            ? Colors.white
                            : gFFI.abModel.getCurrentAbTagColor(name)),
                  ).marginOnly(right: radius / 2),
                  Expanded(
                    child: Text(name,
                        style: TextStyle(
                            overflow: TextOverflow.ellipsis,
                            color: tags.contains(name) ? Colors.white : null)),
                  ),
                ],
              ),
            ),
          )),
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
              if (newName != name &&
                  gFFI.abModel.currentAbTags.contains(newName)) {
                return translate('Already exists');
              }
              return null;
            },
            onSubmit: (String newName) {
              if (name != newName) {
                gFFI.abModel.renameTag(name, newName);
              }
              Future.delayed(Duration.zero, () => Get.back());
            },
            onCancel: () {
              Future.delayed(Duration.zero, () => Get.back());
            });
      }),
      getEntry(translate(translate('Change Color')), () async {
        final model = gFFI.abModel;
        Color oldColor = model.getCurrentAbTagColor(name);
        Color newColor = await showColorPickerDialog(
          context,
          oldColor,
          pickersEnabled: {
            ColorPickerType.accent: false,
            ColorPickerType.wheel: true,
          },
          pickerTypeLabels: {
            ColorPickerType.primary: translate("Primary Color"),
            ColorPickerType.wheel: translate("HSV Color"),
          },
          actionButtons: ColorPickerActionButtons(
              dialogOkButtonLabel: translate("OK"),
              dialogCancelButtonLabel: translate("Cancel")),
          showColorCode: true,
        );
        if (oldColor != newColor) {
          model.setTagColor(name, newColor);
        }
      }),
      getEntry(translate("Delete"), () {
        gFFI.abModel.deleteTag(name);
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

class _RuleTree extends StatefulWidget {
  const _RuleTree();

  @override
  State<_RuleTree> createState() => __RuleTreeState();
}

class __RuleTreeState extends State<_RuleTree> {
  final TreeController _controller = TreeController(allNodesExpanded: true);
  bool mapFetched = false;
  Map<String, List<String>> map = Map.fromEntries([]);
  List<AbRulePayload> rules = [];
  bool isInProgress = false;
  double totalWidth = isDesktop ? 400.0 : 180.0;
  double col1Width = isDesktop ? 300.0 : 100.0;
  double col2Width = 30.0;
  double indent = isDesktop ? 40.0 : 12.0;
  double iconSize = isDesktop ? 24.0 : 12.0;
  double iconButtonSize = 24.0;
  bool onlyShowExisting = false;
  String searchText = '';
  TextStyle? textStyle = isDesktop ? null : TextStyle(fontSize: 12);

  @override
  void initState() {
    super.initState();
    onlyShowExisting =
        bind.getLocalFlutterOption(k: 'only-show-existing-rules') == 'Y';
    refresh();
  }

  void refresh() async {
    setState(() {
      isInProgress = true;
    });
    if (!mapFetched) {
      map = await gFFI.abModel.getNamesTree();
      mapFetched = true;
    }
    final allRules = await gFFI.abModel.getAllRules();
    setState(() {
      isInProgress = false;
      rules = allRules;
    });
  }

  bool match(String name) {
    return searchText.isEmpty ||
        name.toLowerCase().contains(searchText.toLowerCase());
  }

  List<TreeNode> getNodes() {
    int keyIndex = 0;
    List<TreeNode> buildUserNodes(List<String> users) {
      List<TreeNode> userNodes = [];
      for (var user in users) {
        if (!match(user)) {
          continue;
        }
        final userRuleIndex = rules.indexWhere(
            (e) => e.level == ShareLevel.user.value && e.name == user);
        if (userRuleIndex < 0) {
          if (!onlyShowExisting) {
            userNodes.add(TreeNode(
                content: _buildEmptyNodeContent(
                    ShareLevel.user, user, totalWidth, indent * 2),
                key: ValueKey(keyIndex++),
                children: []));
          }
        } else {
          final userRule = rules[userRuleIndex];
          userNodes.add(TreeNode(
              content: _buildRuleNodeContent(userRule, totalWidth, indent * 2),
              key: ValueKey(keyIndex++),
              children: []));
        }
      }
      return userNodes;
    }

    List<TreeNode> groupNodes = [];
    map.forEach((group, users) {
      final groupRuleIndex = rules.indexWhere(
          (e) => e.level == ShareLevel.group.value && e.name == group);
      final children = buildUserNodes(users);
      if (!match(group) && children.isEmpty) {
        return;
      }
      if (groupRuleIndex < 0) {
        if (!onlyShowExisting || children.isNotEmpty) {
          groupNodes.add(TreeNode(
              content: _buildEmptyNodeContent(
                  ShareLevel.group, group, totalWidth, indent),
              key: ValueKey(keyIndex++),
              children: children));
        }
      } else {
        final groupRule = rules[groupRuleIndex];
        groupNodes.add(TreeNode(
            content: _buildRuleNodeContent(groupRule, totalWidth, indent),
            key: ValueKey(keyIndex++),
            children: buildUserNodes(users)));
      }
    });

    List<TreeNode> totalNodes = [];
    final teamRuleIndex =
        rules.indexWhere((e) => e.level == ShareLevel.team.value);
    if (!match(ShareLevel.teamName) && groupNodes.isEmpty) {
      return [];
    }
    if (teamRuleIndex < 0) {
      if (!onlyShowExisting || groupNodes.isNotEmpty) {
        totalNodes.add(TreeNode(
            content: _buildEmptyNodeContent(
                ShareLevel.team, ShareLevel.teamName, totalWidth, 0),
            key: ValueKey(keyIndex++),
            children: groupNodes));
      }
    } else {
      final rule = rules[teamRuleIndex];
      totalNodes.add(TreeNode(
          content: _buildRuleNodeContent(
              AbRulePayload(
                  rule.guid, rule.level, ShareLevel.teamName, rule.rule),
              totalWidth,
              0),
          key: ValueKey(keyIndex++),
          children: groupNodes));
    }
    return totalNodes;
  }

  @override
  Widget build(BuildContext context) {
    Widget switchWidget = Switch(
        value: onlyShowExisting,
        onChanged: (v) {
          setState(() {
            onlyShowExisting = v;
            bind.setLocalFlutterOption(
                k: 'only-show-existing-rules', v: v ? 'Y' : '');
          });
        });
    Widget switchLabel =
        _text(translate('Only show existing')).marginOnly(right: 20);
    Widget searchTextField = TextField(
      decoration: InputDecoration(
        hintText: translate('Search'),
        contentPadding: const EdgeInsets.symmetric(horizontal: 6),
        border: OutlineInputBorder(
          borderRadius: BorderRadius.circular(10.0),
        ),
        prefixIcon: Icon(Icons.search),
        filled: true,
      ),
      onChanged: (v) {
        setState(() {
          searchText = v;
        });
      },
    ).marginSymmetric(horizontal: 10);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.center,
      children: [
        if (isDesktop)
          Row(
            children: [
              switchWidget,
              Expanded(child: switchLabel),
              Expanded(child: searchTextField),
            ],
          ),
        if (!isDesktop)
          Row(
            children: [
              switchWidget,
              Expanded(child: switchLabel),
            ],
          ),
        if (!isDesktop) searchTextField,
        // NOT use Offstage to wrap LinearProgressIndicator
        isInProgress ? const LinearProgressIndicator() : Offstage(),
        SingleChildScrollView(
          scrollDirection: Axis.vertical,
          child: SingleChildScrollView(
            scrollDirection: Axis.horizontal,
            child: TreeView(
              treeController: _controller,
              indent: indent,
              iconSize: iconSize,
              nodes: getNodes(),
            ),
          ),
        ),
      ],
    );
  }

  Widget _buildEmptyNodeContent(
      ShareLevel level, String name, double totalWidth, double indent) {
    return SizedBox(
      width: totalWidth - indent,
      child: Row(
        children: [
          SizedBox(width: col1Width - indent, child: _text(name)),
          SizedBox(width: col2Width),
          const Spacer(),
          if (!onlyShowExisting)
            _iconButton(
              icon: const Icon(Icons.add, color: MyTheme.accent),
              onPressed: () {
                onSubmit(int rule) async {
                  if (ShareRule.fromValue(rule) == null) {
                    BotToast.showText(
                        contentColor: Colors.red, text: "Invalid rule: $rule");
                    return;
                  }
                  setState(() {
                    isInProgress = true;
                  });
                  final errMsg =
                      await gFFI.abModel.addRule(name, level.value, rule);
                  setState(() {
                    isInProgress = false;
                  });
                  if (errMsg != null) {
                    BotToast.showText(contentColor: Colors.red, text: errMsg);
                  } else {
                    refresh();
                  }
                }

                _addOrUpdateRuleDialog(onSubmit, ShareRule.read.value, null);
              },
            )
        ],
      ),
    );
  }

  Widget _buildRuleNodeContent(
      AbRulePayload rule, double totalWidth, double indent) {
    return SizedBox(
      width: totalWidth - indent,
      child: Row(
        children: [
          SizedBox(width: col1Width - indent, child: _text(rule.name)),
          SizedBox(
              width: col2Width, child: _text(ShareRule.shortDesc(rule.rule))),
          const Spacer(),
          Row(
            mainAxisAlignment: MainAxisAlignment.end,
            children: [
              _iconButton(
                icon: const Icon(Icons.edit, color: MyTheme.accent),
                onPressed: () {
                  onSubmit(int v) async {
                    setState(() {
                      isInProgress = true;
                    });
                    final errMsg = await gFFI.abModel.updateRule(rule.guid, v);
                    setState(() {
                      isInProgress = false;
                    });
                    if (errMsg != null) {
                      BotToast.showText(contentColor: Colors.red, text: errMsg);
                    } else {
                      refresh();
                    }
                  }

                  if (ShareRule.fromValue(rule.rule) == null) {
                    BotToast.showText(
                        contentColor: Colors.red,
                        text: "Invalid rule: ${rule.rule}");
                    return;
                  }
                  _addOrUpdateRuleDialog(onSubmit, rule.rule, rule.name);
                },
              ),
              _iconButton(
                icon: const Icon(Icons.delete, color: Colors.red),
                onPressed: () async {
                  onSubmit() async {
                    setState(() {
                      isInProgress = true;
                    });
                    final errMsg = await gFFI.abModel.deleteRules([rule.guid]);
                    setState(() {
                      isInProgress = false;
                    });
                    if (errMsg != null) {
                      BotToast.showText(contentColor: Colors.red, text: errMsg);
                    } else {
                      refresh();
                    }
                  }

                  deleteConfirmDialog(onSubmit, translate('Confirm Delete'));
                },
              ),
            ],
          )
        ],
      ),
    );
  }

  Widget _iconButton({required Widget icon, required VoidCallback? onPressed}) {
    return GestureDetector(
      child:
          SizedBox(width: iconButtonSize, height: iconButtonSize, child: icon),
      onTap: onPressed,
    );
  }

  Text _text(String text) {
    return Text(text, style: textStyle);
  }
}

void _addOrUpdateRuleDialog(
    Future Function(int) onSubmit, int initialRule, String? name) async {
  bool isAdd = name == null;
  var currentRule = initialRule;
  gFFI.dialogManager.show(
    (setState, close, context) {
      submit() async {
        if (ShareRule.fromValue(currentRule) != null) {
          onSubmit(currentRule);
        }
        close();
      }

      final keys = [
        ShareRule.read.value,
        ShareRule.readWrite.value,
        ShareRule.fullControl.value,
      ];
      TextEditingController controller = TextEditingController();
      return CustomAlertDialog(
        contentBoxConstraints: BoxConstraints(maxWidth: 300),
        title: Row(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            Expanded(
              child: Text(
                      '${translate(isAdd ? "Add" : "Update")}${name != null ? " $name" : ""}',
                      overflow: TextOverflow.ellipsis)
                  .paddingOnly(
                left: 10,
              ),
            ),
          ],
        ),
        content: Column(
          crossAxisAlignment: CrossAxisAlignment.center,
          children: [
            DropdownMenu<int>(
              initialSelection: initialRule,
              onSelected: (value) {
                if (value != null) {
                  setState(() {
                    currentRule = value;
                  });
                }
              },
              dropdownMenuEntries: keys
                  .map((e) =>
                      DropdownMenuEntry(value: e, label: ShareRule.desc(e)))
                  .toList(),
              inputDecorationTheme: InputDecorationTheme(
                  isDense: true, border: UnderlineInputBorder()),
              enableFilter: false,
              controller: controller,
            ),
            if (currentRule == ShareRule.fullControl.value)
              Row(
                children: [
                  Icon(Icons.warning_amber, color: Colors.amber)
                      .marginOnly(right: 10),
                  Flexible(
                      child: Text(translate('full_control_tip'),
                          style: TextStyle(fontSize: 12))),
                ],
              ).marginSymmetric(vertical: 10),
          ],
        ),
        actions: [
          dialogButton(
            "Cancel",
            icon: Icon(Icons.close_rounded),
            onPressed: close,
            isOutline: true,
          ),
          dialogButton(
            "OK",
            icon: Icon(Icons.done_rounded),
            onPressed: submit,
          ),
        ],
        onSubmit: submit,
        onCancel: close,
      );
    },
  );
}
