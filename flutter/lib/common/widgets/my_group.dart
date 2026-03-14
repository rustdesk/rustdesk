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
  RxBool get isSelectedDeviceGroup => gFFI.groupModel.isSelectedDeviceGroup;
  RxString get selectedAccessibleItemName =>
      gFFI.groupModel.selectedAccessibleItemName;
  RxString get searchAccessibleItemNameText =>
      gFFI.groupModel.searchAccessibleItemNameText;
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
                    child: _buildLeftList(),
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
                  child: _buildLeftList(),
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
            searchAccessibleItemNameText.value = value;
            selectedAccessibleItemName.value = '';
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

  Widget _buildLeftList() {
    return Obx(() {
      final userItems = gFFI.groupModel.users.where((p0) {
        if (searchAccessibleItemNameText.isNotEmpty) {
          final search = searchAccessibleItemNameText.value.toLowerCase();
          return p0.name.toLowerCase().contains(search) ||
              p0.displayNameOrName.toLowerCase().contains(search);
        }
        return true;
      }).toList();
      // Count occurrences of each displayNameOrName to detect duplicates
      final displayNameCount = <String, int>{};
      for (final u in userItems) {
        final dn = u.displayNameOrName;
        displayNameCount[dn] = (displayNameCount[dn] ?? 0) + 1;
      }
      final deviceGroupItems = gFFI.groupModel.deviceGroups.where((p0) {
        if (searchAccessibleItemNameText.isNotEmpty) {
          return p0.name
              .toLowerCase()
              .contains(searchAccessibleItemNameText.value.toLowerCase());
        }
        return true;
      }).toList();
      listView(bool isPortrait) => ListView.builder(
          shrinkWrap: isPortrait,
          itemCount: deviceGroupItems.length + userItems.length,
          itemBuilder: (context, index) => index < deviceGroupItems.length
              ? _buildDeviceGroupItem(deviceGroupItems[index])
              : _buildUserItem(userItems[index - deviceGroupItems.length],
                  displayNameCount));
      var maxHeight = max(MediaQuery.of(context).size.height / 6, 100.0);
      return Obx(() => stateGlobal.isPortrait.isFalse
          ? listView(false)
          : LimitedBox(maxHeight: maxHeight, child: listView(true)));
    });
  }

  Widget _buildUserItem(UserPayload user, Map<String, int> displayNameCount) {
    final username = user.name;
    final dn = user.displayNameOrName;
    final isDuplicate = (displayNameCount[dn] ?? 0) > 1;
    final displayName =
        isDuplicate && user.displayName.trim().isNotEmpty
            ? '${user.displayName} (@$username)'
            : dn;
    return InkWell(onTap: () {
      isSelectedDeviceGroup.value = false;
      if (selectedAccessibleItemName.value != username) {
        selectedAccessibleItemName.value = username;
      } else {
        selectedAccessibleItemName.value = '';
      }
    }, child: Obx(
      () {
        bool selected = !isSelectedDeviceGroup.value &&
            selectedAccessibleItemName.value == username;
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
                        displayName.characters.first.toUpperCase(),
                        style: TextStyle(color: Colors.white),
                        textAlign: TextAlign.center,
                      ),
                    ),
                  ),
                ).marginOnly(right: 4),
                if (isMe) Flexible(child: Text(displayName)),
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
                if (!isMe) Expanded(child: Text(displayName)),
              ],
            ).paddingSymmetric(vertical: 4),
          ),
        );
      },
    )).marginSymmetric(horizontal: 12).marginOnly(bottom: 6);
  }

  Widget _buildDeviceGroupItem(DeviceGroupPayload deviceGroup) {
    final name = deviceGroup.name;
    return InkWell(onTap: () {
      isSelectedDeviceGroup.value = true;
      if (selectedAccessibleItemName.value != name) {
        selectedAccessibleItemName.value = name;
      } else {
        selectedAccessibleItemName.value = '';
      }
    }, child: Obx(
      () {
        bool selected = isSelectedDeviceGroup.value &&
            selectedAccessibleItemName.value == name;
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
                  child: Icon(IconFont.deviceGroupOutline,
                      color: MyTheme.accent, size: 19),
                ).marginOnly(right: 4),
                Expanded(child: Text(name)),
              ],
            ).paddingSymmetric(vertical: 4),
          ),
        );
      },
    )).marginSymmetric(horizontal: 12).marginOnly(bottom: 6);
  }
}
