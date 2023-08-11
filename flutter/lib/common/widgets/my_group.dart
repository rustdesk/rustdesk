import 'package:flutter/material.dart';
import 'package:flutter_hbb/common/hbbs/hbbs.dart';
import 'package:flutter_hbb/common/widgets/login.dart';
import 'package:flutter_hbb/common/widgets/peers_view.dart';
import 'package:get/get.dart';

import '../../common.dart';

class MyGroup extends StatefulWidget {
  final EdgeInsets? menuPadding;
  const MyGroup({Key? key, this.menuPadding}) : super(key: key);

  @override
  State<StatefulWidget> createState() {
    return _MyGroupState();
  }
}

class _MyGroupState extends State<MyGroup> {
  RxString get selectedUser => gFFI.groupModel.selectedUser;
  RxString get searchUserText => gFFI.groupModel.searchUserText;
  static TextEditingController searchUserController = TextEditingController();

  @override
  void initState() {
    super.initState();
  }

  @override
  Widget build(BuildContext context) {
    return Obx(() {
      // use username to be same with ab
      if (gFFI.userModel.userName.value.isEmpty) {
        return Center(
            child: ElevatedButton(
                onPressed: loginDialog, child: Text(translate("Login"))));
      }
      return buildBody(context);
    });
  }

  Widget buildBody(BuildContext context) {
    return Obx(() {
      if (gFFI.groupModel.groupLoading.value) {
        return const Center(
          child: CircularProgressIndicator(),
        );
      }
      if (gFFI.groupModel.groupLoadError.isNotEmpty) {
        return _buildShowError(gFFI.groupModel.groupLoadError.value);
      }
      if (isDesktop) {
        return _buildDesktop();
      } else {
        return _buildMobile();
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
              gFFI.groupModel.pull();
            },
            child: Text(translate("Retry")))
      ],
    ));
  }

  Widget _buildDesktop() {
    return Row(
      children: [
        Container(
          decoration: BoxDecoration(
              borderRadius: BorderRadius.circular(12),
              border:
                  Border.all(color: Theme.of(context).colorScheme.background)),
          child: Container(
            width: 150,
            height: double.infinity,
            child: Column(
              children: [
                _buildLeftHeader(),
                Expanded(
                  child: Container(
                    width: double.infinity,
                    height: double.infinity,
                    child: _buildUserContacts(),
                  ),
                )
              ],
            ),
          ),
        ).marginOnly(right: 12.0),
        Expanded(
          child: Align(
              alignment: Alignment.topLeft,
              child: Obx(() => MyGroupPeerView(
                  menuPadding: widget.menuPadding,
                  // ignore: invalid_use_of_protected_member
                  initPeers: gFFI.groupModel.peersShow.value))),
        )
      ],
    );
  }

  Widget _buildMobile() {
    return Column(
      children: [
        Container(
          decoration: BoxDecoration(
              borderRadius: BorderRadius.circular(6),
              border:
                  Border.all(color: Theme.of(context).colorScheme.background)),
          child: Container(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                _buildLeftHeader(),
                Container(
                  width: double.infinity,
                  child: _buildUserContacts(),
                )
              ],
            ),
          ),
        ).marginOnly(bottom: 12.0),
        Expanded(
          child: Align(
              alignment: Alignment.topLeft,
              child: Obx(() => MyGroupPeerView(
                  menuPadding: widget.menuPadding,
                  // ignore: invalid_use_of_protected_member
                  initPeers: gFFI.groupModel.peersShow.value))),
        )
      ],
    );
  }

  Widget _buildLeftHeader() {
    return Row(
      children: [
        Expanded(
            child: TextField(
          controller: searchUserController,
          onChanged: (value) {
            searchUserText.value = value;
          },
          decoration: InputDecoration(
            filled: false,
            prefixIcon: Icon(
              Icons.search_rounded,
              color: Theme.of(context).hintColor,
            ),
            contentPadding: const EdgeInsets.symmetric(vertical: 10),
            hintText: translate("Search"),
            hintStyle:
                TextStyle(fontSize: 14, color: Theme.of(context).hintColor),
            border: InputBorder.none,
            isDense: true,
          ),
        )),
      ],
    );
  }

  Widget _buildUserContacts() {
    return Obx(() {
      return Column(
          children: gFFI.groupModel.users
              .where((p0) {
                if (searchUserText.isNotEmpty) {
                  return p0.name.contains(searchUserText.value);
                }
                return true;
              })
              .map((e) => _buildUserItem(e))
              .toList());
    });
  }

  Widget _buildUserItem(UserPayload user) {
    final username = user.name;
    return InkWell(onTap: () {
      if (selectedUser.value != username) {
        selectedUser.value = username;
      } else {
        selectedUser.value = '';
      }
    }, child: Obx(
      () {
        bool selected = selectedUser.value == username;
        return Container(
          decoration: BoxDecoration(
            color: selected ? MyTheme.color(context).highlight : null,
            border: Border(
                bottom: BorderSide(
                    width: 0.7,
                    color: Theme.of(context).dividerColor.withOpacity(0.1))),
          ),
          child: Container(
            child: Row(
              children: [
                Icon(Icons.person_rounded, color: Colors.grey, size: 16)
                    .marginOnly(right: 4),
                Expanded(child: Text(username)),
              ],
            ).paddingSymmetric(vertical: 4),
          ),
        );
      },
    )).marginSymmetric(horizontal: 12).marginOnly(bottom: 6);
  }
}
