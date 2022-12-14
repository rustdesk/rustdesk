import 'package:flutter/material.dart';
import 'package:flutter_hbb/common/formatter/id_formatter.dart';
import 'package:flutter_hbb/common/widgets/peer_card.dart';
import 'package:flutter_hbb/common/widgets/peers_view.dart';
import 'package:flutter_hbb/desktop/widgets/popup_menu.dart';
import 'package:flutter_hbb/desktop/widgets/login.dart';
import '../../consts.dart';
import '../../desktop/widgets/material_mod_popup_menu.dart' as mod_menu;
import 'package:get/get.dart';

import '../../common.dart';
import '../../desktop/pages/desktop_home_page.dart';
import '../../mobile/pages/settings_page.dart';

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
  Widget build(BuildContext context) => FutureBuilder<Widget>(
      future: buildBody(context),
      builder: (context, snapshot) {
        if (snapshot.hasData) {
          return snapshot.data!;
        } else {
          return const Offstage();
        }
      });

  handleLogin() {
    // TODO refactor login dialog for desktop and mobile
    if (isDesktop) {
      loginDialog();
    } else {
      showLogin(gFFI.dialogManager);
    }
  }

  Future<Widget> buildBody(BuildContext context) async {
    return Obx(() {
      if (gFFI.userModel.userName.value.isEmpty) {
        return Center(
          child: InkWell(
            onTap: handleLogin,
            child: Text(
              translate("Login"),
              style: const TextStyle(decoration: TextDecoration.underline),
            ),
          ),
        );
      } else {
        if (gFFI.abModel.abLoading.value) {
          return const Center(
            child: CircularProgressIndicator(),
          );
        }
        if (gFFI.abModel.abError.isNotEmpty) {
          return _buildShowError(gFFI.abModel.abError.value);
        }
        return isDesktop
            ? _buildAddressBookDesktop()
            : _buildAddressBookMobile();
      }
    });
  }

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

  Widget _buildAddressBookDesktop() {
    return Row(
      children: [
        Card(
          margin: EdgeInsets.symmetric(horizontal: 4.0),
          shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(12),
              side:
                  BorderSide(color: Theme.of(context).scaffoldBackgroundColor)),
          child: Container(
            width: 200,
            height: double.infinity,
            padding:
                const EdgeInsets.symmetric(horizontal: 12.0, vertical: 8.0),
            child: Column(
              children: [
                _buildTagHeader(),
                Expanded(
                  child: Container(
                    width: double.infinity,
                    height: double.infinity,
                    decoration: BoxDecoration(
                        border: Border.all(color: MyTheme.darkGray),
                        borderRadius: BorderRadius.circular(2)),
                    child: _buildTags(),
                  ).marginSymmetric(vertical: 8.0),
                )
              ],
            ),
          ),
        ).marginOnly(right: 8.0),
        _buildPeersViews()
      ],
    );
  }

  Widget _buildAddressBookMobile() {
    return Column(
      children: [
        Card(
          margin: EdgeInsets.symmetric(horizontal: 1.0),
          shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(6),
              side:
                  BorderSide(color: Theme.of(context).scaffoldBackgroundColor)),
          child: Container(
            padding:
                const EdgeInsets.symmetric(horizontal: 12.0, vertical: 8.0),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                _buildTagHeader(),
                Container(
                  width: double.infinity,
                  decoration: BoxDecoration(
                      border: Border.all(color: MyTheme.darkGray),
                      borderRadius: BorderRadius.circular(4)),
                  child: _buildTags(),
                ).marginSymmetric(vertical: 8.0),
              ],
            ),
          ),
        ),
        Divider(),
        _buildPeersViews()
      ],
    );
  }

  Widget _buildTagHeader() {
    return Row(
      mainAxisAlignment: MainAxisAlignment.spaceBetween,
      children: [
        Text(translate('Tags')),
        GestureDetector(
            onTapDown: (e) {
              final x = e.globalPosition.dx;
              final y = e.globalPosition.dy;
              menuPos = RelativeRect.fromLTRB(x, y, x, y);
            },
            onTap: () => _showMenu(menuPos),
            child: ActionMore()),
      ],
    );
  }

  Widget _buildTags() {
    return Obx(
      () => Wrap(
        children: gFFI.abModel.tags
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
      ),
    );
  }

  Widget _buildPeersViews() {
    return Expanded(
      child: Align(
          alignment: Alignment.topLeft,
          child: Obx(() => AddressBookPeersView(
                menuPadding: widget.menuPadding,
                initPeers: gFFI.abModel.peers.value,
              ))),
    );
  }

  void _showMenu(RelativeRect pos) {
    final items = [
      getEntry(translate("Add ID"), abAddId),
      getEntry(translate("Add Tag"), abAddTag),
      getEntry(translate("Unselect all tags"), gFFI.abModel.unsetSelectedTags),
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
    var isInProgress = false;
    IDTextEditingController idController = IDTextEditingController(text: '');
    TextEditingController aliasController = TextEditingController(text: '');
    final tags = List.of(gFFI.abModel.tags);
    var selectedTag = List<dynamic>.empty(growable: true).obs;
    final style = TextStyle(fontSize: 14.0);
    String? errorMsg;

    gFFI.dialogManager.show((setState, close) {
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
                  decoration: InputDecoration(
                      isDense: true,
                      border: OutlineInputBorder(),
                      errorText: errorMsg),
                  style: style,
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
                  decoration: InputDecoration(
                    border: OutlineInputBorder(),
                    isDense: true,
                  ),
                  style: style,
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
          TextButton(onPressed: close, child: Text(translate("Cancel"))),
          TextButton(onPressed: submit, child: Text(translate("OK"))),
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
    gFFI.dialogManager.show((setState, close) {
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
                      border: const OutlineInputBorder(),
                      errorText: msg.isEmpty ? null : translate(msg),
                    ),
                    controller: controller,
                    focusNode: FocusNode()..requestFocus(),
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
          TextButton(onPressed: close, child: Text(translate("Cancel"))),
          TextButton(onPressed: submit, child: Text(translate("OK"))),
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
              color: tags.contains(name) ? Colors.blue : null,
              border: Border.all(color: MyTheme.darkGray),
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
    padding: kDesktopMenuPadding,
    dismissOnClicked: true,
  );
}
