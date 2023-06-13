import 'dart:math';
import 'package:flutter/material.dart';
import 'package:get/get.dart';
import 'package:intl/intl.dart';

import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_hbb/models/server_model.dart';

import 'package:flutter_hbb/common/widgets/dialogs/widgets/title.dart';
import 'package:flutter_hbb/common/widgets/dialogs/widgets/buttons.dart';
import 'package:flutter_hbb/desktop/pages/settings/widgets/section.dart';

class SectionTrustedClients extends StatefulWidget {
  SectionTrustedClients({Key? key}) : super(key: key);

  @override
  SectionTrustedClientsState createState() => SectionTrustedClientsState();
}

class SectionTrustedClientsState extends State<SectionTrustedClients> {
  final RxList clients = [].obs;

  // TODO remove, dev only ---------------------------------------------
  String generateName(int len) {
    var r = Random();
    const chars =
        'AaBbCcDdEeFfGgHhIiJjKkLlMmNnOoPpQqRrSsTtUuVvWwXxYyZz1234567890';
    return List.generate(len, (index) => chars[r.nextInt(chars.length)]).join();
  }

  String generateId(int len) {
    var r = Random();
    const chars = '1234567890';
    return List.generate(len, (index) => chars[r.nextInt(chars.length)]).join();
  }

  Future<void> add() async {
    TrustedClient client =
        TrustedClient(generateName(15), generateId(7), DateTime.now());
    TrustedClientsController.add(client).then((_) => clients.add(client));
  }
  // remove, dev only ------------------------------------------------

  Future<void> remove(client) async {
    gFFI.dialogManager.show(
      (setState, close, context) {
        submit() async {
          TrustedClientsController.remove(client)
              .then((_) => clients.remove(client));
          close();
        }

        return CustomAlertDialog(
          contentBoxConstraints: const BoxConstraints(maxWidth: 350),
          title: dialogTitleDelete(),
          content: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text('Do you want to delete this client?').marginOnly(bottom: 16),
              Container(
                padding: EdgeInsets.fromLTRB(10, 8, 8, 8),
                decoration: BoxDecoration(
                  color: Theme.of(context).brightness == Brightness.dark
                      ? Color.fromARGB(135, 87, 87, 90)
                      : Colors.grey[100],
                  borderRadius: BorderRadius.circular(8),
                  border: Border.all(color: Colors.grey),
                ),
                child: Row(
                  children: [
                    CircleAvatar(
                      backgroundColor: str2color(
                          client.name,
                          Theme.of(context).brightness == Brightness.light
                              ? 255
                              : 150),
                      child: Text(client.name[0]),
                    ).marginOnly(right: 15),
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            client.name,
                            style: const TextStyle(fontSize: 18),
                          ).marginOnly(bottom: 5),
                          Row(
                            children: [
                              Text(
                                'ID:',
                                style: const TextStyle(fontSize: 12),
                              ).marginOnly(right: 2),
                              Text(
                                client.peerId,
                                style: const TextStyle(
                                    fontSize: 12, fontWeight: FontWeight.bold),
                              ).marginOnly(right: 8)
                            ],
                          )
                        ],
                      ),
                    ),
                  ],
                ),
              ).marginOnly(bottom: 16),
              Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                mainAxisAlignment: MainAxisAlignment.center,
                children: [
                  Icon(Icons.info_outline_rounded, size: 24)
                      .marginOnly(right: 8),
                  Expanded(
                    child: Text(
                      'Next time this client is connected, you will asked again to add it to the trusted clients.',
                      style: TextStyle(fontSize: 12),
                    ),
                  ),
                ],
              )
            ],
          ),
          actions: [
            dialogCancelButton(onPressed: close),
            dialogSubmitButton(text: 'Yes', onPressed: submit),
          ],
          onSubmit: submit,
          onCancel: close,
        );
      },
    );
  }

  Future<void> load() async {
    TrustedClientsController.getClients().then((trustedClients) {
      clients.addAll(trustedClients);
    });
  }

  @override
  void initState() {
    load();
    super.initState();
  }

  @override
  Widget build(BuildContext context) {
    return SettingsSection(
      title: 'settings.section.trusted_clients.title',
      children: [
        Obx(
          () => Column(
            children: [
              ...clients.map((client) => TrustedClientListItem(
                    client,
                    () => remove(client),
                  ).marginOnly(bottom: 8)),
              // TODO remove, dev only ---------------------------------------------
              ElevatedButton(onPressed: add, child: Text('dev add'))
            ],
          ),
        )
      ],
    );
  }
}

class TrustedClientListItem extends StatelessWidget {
  final TrustedClient client;
  final VoidCallback removeClient;

  TrustedClientListItem(this.client, this.removeClient);

  Future getLocale() async {
    return bind.mainGetLocalOption(key: kCommConfKeyLang);
  }

  @override
  Widget build(BuildContext context) {
    Rx<String> dateStr = ''.obs;

    getLocale().then((locale) {
      dateStr.value = [
        DateFormat.yMMMd(locale).format(client.time),
        DateFormat.Hms(locale).format(client.time)
      ].join(' ');
    });

    return Container(
      padding: EdgeInsets.fromLTRB(10, 8, 8, 8),
      decoration: BoxDecoration(
        color: Theme.of(context).brightness == Brightness.dark
            ? Color.fromARGB(135, 87, 87, 90)
            : Colors.grey[100],
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: Colors.grey),
      ),
      child: Row(
        children: [
          CircleAvatar(
            backgroundColor: str2color(client.name,
                Theme.of(context).brightness == Brightness.light ? 255 : 150),
            child: Text(client.name[0]),
          ).marginOnly(right: 15),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  client.name,
                  style: const TextStyle(fontSize: 18),
                ).marginOnly(bottom: 5),
                Row(
                  children: [
                    Text(
                      'ID:',
                      style: const TextStyle(fontSize: 12),
                    ).marginOnly(right: 2),
                    Text(
                      client.peerId,
                      style: const TextStyle(
                          fontSize: 12, fontWeight: FontWeight.bold),
                    ).marginOnly(right: 8)
                  ],
                ).marginOnly(bottom: 5),
                Row(
                  children: [
                    Text(
                      'Added:',
                      style: const TextStyle(fontSize: 12),
                    ).marginOnly(right: 2),
                    Obx(
                      () => Text(
                        dateStr.value,
                        style: const TextStyle(fontSize: 12),
                      ),
                    )
                  ],
                )
              ],
            ),
          ),
          // TODO use button component
          ElevatedButton.icon(
            icon: Icon(Icons.delete_outline_rounded, size: 16),
            style: ElevatedButton.styleFrom(
              padding: EdgeInsets.fromLTRB(12, 15, 16, 15),
              backgroundColor: Colors.red,
              textStyle: TextStyle(fontSize: 15, fontWeight: FontWeight.normal),
            ),
            onPressed: removeClient,
            label: Text(translate('Delete')),
          )
        ],
      ),
    );
  }
}
