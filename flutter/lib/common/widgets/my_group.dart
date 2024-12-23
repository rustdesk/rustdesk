import 'dart:math';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/common/hbbs/hbbs.dart';
import 'package:flutter_hbb/common/widgets/login.dart';
import 'package:flutter_hbb/common/widgets/peers_view.dart';
import 'package:flutter_hbb/models/state_model.dart';
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
  Widget build(BuildContext context) {
    return Obx(() {
      if (!gFFI.userModel.isLogin) {
        return Center(
            child: ElevatedButton(
                onPressed: loginDialog, child: Text(translate("Login"))));
      } else if (gFFI.userModel.networkError.isNotEmpty) {
        return netWorkErrorWidget();
      } else if (gFFI.groupModel.groupLoading.value && gFFI.groupModel.emtpy) {
        return const Center(
          child: CircularProgressIndicator(),
        );
      }
      return Column(
        children: [
          buildErrorBanner(context,
              loading: gFFI.groupModel.groupLoading,
              err: gFFI.groupModel.groupLoadError,
              retry: null,
              close: () => gFFI.groupModel.groupLoadError.value = ''),
          Expanded(
              child: Obx(() => stateGlobal.isPortrait.isTrue
                  ? _buildPortrait()
                  : _buildLandscape())),
        ],
      );
    });
  }

  Widget _buildLandscape() {
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
              child: MyGroupPeerView(
                menuPadding: widget.menuPadding,
              )),
        )
      ],
    );
  }

  Widget _buildPortrait() {
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
              child: MyGroupPeerView(
                menuPadding: widget.menuPadding,
              )),
        )
      ],
    );
  }

  Widget _buildLeftHeader() {
    final fontSize = 14.0;
    return Row(
      children: [
        Expanded(
            child: TextField(
          controller: searchUserController,
          onChanged: (value) {
            searchUserText.value = value;
          },
          textAlignVertical: TextAlignVertical.center,
          style: TextStyle(fontSize: fontSize),
          decoration: InputDecoration(
            filled: false,
            prefixIcon: Icon(
              Icons.search_rounded,
              color: Theme.of(context).hintColor,
            ).paddingOnly(top: 2),
            hintText: translate("Search"),
            hintStyle: TextStyle(fontSize: fontSize),
            border: InputBorder.none,
            isDense: true,
          ),
        ).workaroundFreezeLinuxMint()),
      ],
    );
  }

  Widget _buildUserContacts() {
    return Obx(() {
      final items = gFFI.groupModel.users.where((p0) {
        if (searchUserText.isNotEmpty) {
          return p0.name
              .toLowerCase()
              .contains(searchUserText.value.toLowerCase());
        }
        return true;
      }).toList();
      listView(bool isPortrait) => ListView.builder(
          shrinkWrap: isPortrait,
          itemCount: items.length,
          itemBuilder: (context, index) => _buildUserItem(items[index]));
      var maxHeight = max(MediaQuery.of(context).size.height / 6, 100.0);
      return Obx(() => stateGlobal.isPortrait.isFalse
          ? listView(false)
          : LimitedBox(maxHeight: maxHeight, child: listView(true)));
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
        final isMe = username == gFFI.userModel.userName.value;
        final colorMe = MyTheme.color(context).me!;
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
                Container(
                  width: 20,
                  height: 20,
                  decoration: BoxDecoration(
                    color: str2color(username, 0xAF),
                    shape: BoxShape.circle,
                  ),
                  child: Align(
                    alignment: Alignment.center,
                    child: Center(
                      child: Text(
                        username.characters.first.toUpperCase(),
                        style: TextStyle(color: Colors.white),
                        textAlign: TextAlign.center,
                      ),
                    ),
                  ),
                ).marginOnly(right: 4),
                if (isMe) Flexible(child: Text(username)),
                if (isMe)
                  Flexible(
                    child: Container(
                      margin: EdgeInsets.only(left: 5),
                      padding: EdgeInsets.symmetric(horizontal: 3, vertical: 1),
                      decoration: BoxDecoration(
                          color: colorMe.withAlpha(20),
                          borderRadius: BorderRadius.all(Radius.circular(2)),
                          border: Border.all(color: colorMe.withAlpha(100))),
                      child: Text(
                        translate('Me'),
                        style: TextStyle(
                            color: colorMe.withAlpha(200), fontSize: 12),
                      ),
                    ),
                  ),
                if (!isMe) Expanded(child: Text(username)),
              ],
            ).paddingSymmetric(vertical: 4),
          ),
        );
      },
    )).marginSymmetric(horizontal: 12).marginOnly(bottom: 6);
  }
}
