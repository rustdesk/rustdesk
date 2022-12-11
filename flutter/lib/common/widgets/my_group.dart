import 'package:flutter/material.dart';
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
  static final RxString selectedUser = ''.obs;
  static final RxString searchUserText = ''.obs;
  static TextEditingController searchUserController = TextEditingController();

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

  Future<Widget> buildBody(BuildContext context) async {
    return Obx(() {
      if (gFFI.groupModel.userLoading.value) {
        return const Center(
          child: CircularProgressIndicator(),
        );
      }
      if (gFFI.groupModel.userLoadError.isNotEmpty) {
        return _buildShowError(gFFI.groupModel.userLoadError.value);
      }
      return Row(
        children: [
          _buildLeftDesktop(),
          Expanded(
            child: Align(
                alignment: Alignment.topLeft,
                child: MyGroupPeerView(
                    menuPadding: widget.menuPadding,
                    initPeers: gFFI.groupModel.peersShow.value)),
          )
        ],
      );
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

  Widget _buildLeftDesktop() {
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
                _buildLeftHeader(),
                Expanded(
                  child: Container(
                    width: double.infinity,
                    height: double.infinity,
                    decoration:
                        BoxDecoration(borderRadius: BorderRadius.circular(2)),
                    child: _buildUserContacts(),
                  ).marginSymmetric(vertical: 8.0),
                )
              ],
            ),
          ),
        ).marginOnly(right: 8.0),
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
              .map((e) => _buildUserItem(e.name))
              .toList());
    });
  }

  Widget _buildUserItem(String username) {
    return InkWell(onTap: () {
      if (selectedUser.value != username) {
        selectedUser.value = username;
        gFFI.groupModel.pullUserPeers(username);
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
                Icon(Icons.person_outline_rounded, color: Colors.grey, size: 16)
                    .marginOnly(right: 4),
                Expanded(child: Text(username)),
              ],
            ).paddingSymmetric(vertical: 4),
          ),
        );
      },
    )).marginSymmetric(horizontal: 12);
  }
}
