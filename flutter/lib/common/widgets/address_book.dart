import 'package:contextmenu/contextmenu.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common/widgets/peers_view.dart';
import 'package:get/get.dart';

import '../../common.dart';
import '../../desktop/pages/desktop_home_page.dart';
import '../../mobile/pages/settings_page.dart';
import '../../models/platform_model.dart';

class AddressBook extends StatefulWidget {
  final EdgeInsets? menuPadding;
  const AddressBook({Key? key, this.menuPadding}) : super(key: key);

  @override
  State<StatefulWidget> createState() {
    return _AddressBookState();
  }
}

class _AddressBookState extends State<AddressBook> {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) => gFFI.abModel.pullAb());
  }

  @override
  Widget build(BuildContext context) => FutureBuilder<Widget>(
      future: buildAddressBook(context),
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
      loginDialog().then((success) {
        if (success) {
          setState(() {});
        }
      });
    } else {
      showLogin(gFFI.dialogManager);
    }
  }

  Future<Widget> buildAddressBook(BuildContext context) async {
    final token = await bind.mainGetLocalOption(key: 'access_token');
    if (token.trim().isEmpty) {
      return Center(
        child: InkWell(
          onTap: handleLogin,
          child: Text(
            translate("Login"),
            style: const TextStyle(decoration: TextDecoration.underline),
          ),
        ),
      );
    }
    final model = gFFI.abModel;
    return FutureBuilder(
        future: model.pullAb(),
        builder: (context, snapshot) {
          if (snapshot.hasData) {
            return _buildAddressBook(context);
          } else if (snapshot.hasError) {
            return _buildShowError(snapshot.error.toString());
          } else {
            return Obx(() {
              if (model.abLoading.value) {
                return const Center(
                  child: CircularProgressIndicator(),
                );
              } else if (model.abError.isNotEmpty) {
                return _buildShowError(model.abError.value);
              } else {
                return const Offstage();
              }
            });
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
              setState(() {});
            },
            child: Text(translate("Retry")))
      ],
    ));
  }

  Widget _buildAddressBook(BuildContext context) {
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
                Row(
                  mainAxisAlignment: MainAxisAlignment.spaceBetween,
                  children: [
                    // TODO same style as peer
                    Text(translate('Tags')),
                    InkWell(
                      child: PopupMenuButton(
                          itemBuilder: (context) => [
                                PopupMenuItem(
                                  value: 'add-id',
                                  child: Text(translate("Add ID")),
                                ),
                                PopupMenuItem(
                                  value: 'add-tag',
                                  child: Text(translate("Add Tag")),
                                ),
                                PopupMenuItem(
                                  value: 'unset-all-tag',
                                  child: Text(translate("Unselect all tags")),
                                ),
                              ],
                          onSelected: handleAbOp,
                          child: const Icon(Icons.more_vert_outlined)),
                    )
                  ],
                ),
                Expanded(
                  child: Container(
                    width: double.infinity,
                    height: double.infinity,
                    decoration: BoxDecoration(
                        border: Border.all(color: MyTheme.darkGray),
                        borderRadius: BorderRadius.circular(2)),
                    child: Obx(
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
                    ),
                  ).marginSymmetric(vertical: 8.0),
                )
              ],
            ),
          ),
        ).marginOnly(right: 8.0),
        Expanded(
          child: Align(
              alignment: Alignment.topLeft,
              child: Obx(() => AddressBookPeersView(
                    menuPadding: widget.menuPadding,
                    initPeers: gFFI.abModel.peers.value,
                  ))),
        )
      ],
    );
  }

  /// tag operation
  void handleAbOp(String value) {
    if (value == 'add-id') {
      abAddId();
    } else if (value == 'add-tag') {
      abAddTag();
    } else if (value == 'unset-all-tag') {
      gFFI.abModel.unsetSelectedTags();
    }
  }

  void abAddId() async {
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
          final ids = field.trim().split(RegExp(r"[\s,;\n]+"));
          field = ids.join(',');
          for (final newId in ids) {
            if (gFFI.abModel.idContainBy(newId)) {
              continue;
            }
            gFFI.abModel.addId(newId);
          }
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
                      focusNode: FocusNode()..requestFocus()),
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
  final bool useContextMenuArea;

  const AddressBookTag(
      {Key? key,
      required this.name,
      required this.tags,
      this.onTap,
      this.useContextMenuArea = true})
      : super(key: key);

  @override
  Widget build(BuildContext context) {
    final body = GestureDetector(
      onTap: onTap,
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
    return useContextMenuArea
        ? ContextMenuArea(
            width: 100,
            builder: (context) => [
              ListTile(
                title: Text(translate("Delete")),
                onTap: () {
                  gFFI.abModel.deleteTag(name);
                  gFFI.abModel.pushAb();
                  Future.delayed(Duration.zero, () => Get.back());
                },
              )
            ],
            child: body,
          )
        : body;
  }
}
