import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_hbb/utils/http_service.dart' as http;

const easyAccessDescriptionText =
    'View users and groups with easy access permission for this device.';
const noEasyAccessManagersText = 'No easy access managers';
const noEasyAccessUsersText = 'No users';
const noEasyAccessUserGroupsText = 'No user groups';

typedef EasyAccessEntriesBuilder = Widget Function(
  BuildContext context,
  List<Map<String, dynamic>> entries,
  String emptyText,
);

bool isAllowEasyAccess() =>
    bind.mainGetHardOption(key: 'allow-easy-access') == 'Y';

bool isEasyAccessUserGroupType(dynamic type) => type == 1 || type == 3;

bool isEasyAccessUserType(dynamic type) => type == 2 || type == 4;

List<Map<String, dynamic>> mergeEasyAccessEntriesByName(
  List<Map<String, dynamic>> entries,
) {
  final merged = <String, Map<String, dynamic>>{};
  for (final entry in entries) {
    final name = (entry['name'] ?? '').toString().trim();
    if (name.isEmpty) {
      continue;
    }
    final key = name.toLowerCase();
    merged.putIfAbsent(key, () {
      final normalizedEntry = Map<String, dynamic>.from(entry);
      normalizedEntry['name'] = name;
      return normalizedEntry;
    });
  }

  final deduped = merged.values.toList();
  deduped.sort((a, b) => (a['name'] ?? '').toString().toLowerCase().compareTo(
        (b['name'] ?? '').toString().toLowerCase(),
      ));
  return deduped;
}

Future<List<Map<String, dynamic>>> fetchEasyAccessManagers() async {
  try {
    final authBody = await bind.mainGetEasyAccessDeviceAuth();
    if (authBody.isEmpty) {
      return [];
    }

    final url = await bind.mainGetApiServer();
    if (url.isEmpty) {
      return [];
    }

    final response = await http.post(
      Uri.parse('$url/api/devices/easy-access-managers'),
      headers: {'Content-Type': 'application/json'},
      body: authBody,
    );
    if (response.statusCode != 200) {
      return [];
    }

    final List<dynamic> data = jsonDecode(response.body);
    return data
        .whereType<Map>()
        .map((entry) => Map<String, dynamic>.from(entry))
        .toList();
  } catch (e) {
    debugPrint('Failed to fetch easy access managers: $e');
    return [];
  }
}

List<Map<String, dynamic>> easyAccessUsers(List<Map<String, dynamic>> entries) {
  return mergeEasyAccessEntriesByName(
    entries.where((entry) => isEasyAccessUserType(entry['type'])).toList(),
  );
}

List<Map<String, dynamic>> easyAccessUserGroups(
  List<Map<String, dynamic>> entries,
) {
  return mergeEasyAccessEntriesByName(
    entries.where((entry) => isEasyAccessUserGroupType(entry['type'])).toList(),
  );
}

class EasyAccessContent extends StatelessWidget {
  const EasyAccessContent({
    super.key,
    required this.entryBuilder,
    this.padding = EdgeInsets.zero,
    this.expandTabView = true,
    this.tabViewHeight = 190,
  });

  final EasyAccessEntriesBuilder entryBuilder;
  final EdgeInsetsGeometry padding;
  final bool expandTabView;
  final double tabViewHeight;

  @override
  Widget build(BuildContext context) {
    return FutureBuilder<List<Map<String, dynamic>>>(
      future: fetchEasyAccessManagers(),
      builder: (context, snapshot) {
        if (snapshot.connectionState != ConnectionState.done) {
          return const Padding(
            padding: EdgeInsets.all(20),
            child: Center(
              child: CircularProgressIndicator(),
            ),
          );
        }

        final entries = (snapshot.data ?? const <Map<String, dynamic>>[])
            .map((entry) => Map<String, dynamic>.from(entry))
            .toList();
        if (entries.isEmpty) {
          return Padding(
            padding: const EdgeInsets.all(20),
            child: Center(
              child: Text(translate(noEasyAccessManagersText)),
            ),
          );
        }

        final content = DefaultTabController(
          length: 2,
          child: Column(
            mainAxisSize: expandTabView ? MainAxisSize.max : MainAxisSize.min,
            children: [
              TabBar(
                tabs: [
                  Tab(text: translate('Users')),
                  Tab(text: translate('User Groups')),
                ],
              ),
              const SizedBox(height: 12),
              if (expandTabView)
                Expanded(
                  child: TabBarView(
                    children: [
                      entryBuilder(
                        context,
                        easyAccessUsers(entries),
                        noEasyAccessUsersText,
                      ),
                      entryBuilder(
                        context,
                        easyAccessUserGroups(entries),
                        noEasyAccessUserGroupsText,
                      ),
                    ],
                  ),
                )
              else
                SizedBox(
                  height: tabViewHeight,
                  child: TabBarView(
                    children: [
                      entryBuilder(
                        context,
                        easyAccessUsers(entries),
                        noEasyAccessUsersText,
                      ),
                      entryBuilder(
                        context,
                        easyAccessUserGroups(entries),
                        noEasyAccessUserGroupsText,
                      ),
                    ],
                  ),
                ),
            ],
          ),
        );

        return Padding(
          padding: padding,
          child: content,
        );
      },
    );
  }
}

class EasyAccessTable extends StatelessWidget {
  const EasyAccessTable({
    super.key,
    required this.entries,
    required this.emptyText,
  });

  final List<Map<String, dynamic>> entries;
  final String emptyText;

  @override
  Widget build(BuildContext context) {
    if (entries.isEmpty) {
      return Padding(
        padding: const EdgeInsets.all(20),
        child: Text(translate(emptyText)),
      );
    }

    final theme = Theme.of(context);
    final borderColor = theme.dividerColor;
    final headerColor = theme.colorScheme.surfaceVariant;
    final headerTextStyle = theme.textTheme.bodyMedium?.copyWith(
      color: theme.colorScheme.onSurfaceVariant,
      fontWeight: FontWeight.bold,
    );

    return SingleChildScrollView(
      child: Table(
        border: TableBorder.all(color: borderColor),
        children: [
          TableRow(
            decoration: BoxDecoration(color: headerColor),
            children: [
              Padding(
                padding: const EdgeInsets.all(8),
                child: Text(translate('Name'), style: headerTextStyle),
              ),
            ],
          ),
          ...entries.map(
            (entry) => TableRow(
              children: [
                Padding(
                  padding: const EdgeInsets.all(8),
                  child: Text((entry['name'] ?? '').toString()),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

class EasyAccessNameList extends StatelessWidget {
  const EasyAccessNameList({
    super.key,
    required this.entries,
    required this.emptyText,
  });

  final List<Map<String, dynamic>> entries;
  final String emptyText;

  @override
  Widget build(BuildContext context) {
    if (entries.isEmpty) {
      return Center(child: Text(translate(emptyText)));
    }

    return ListView.separated(
      padding: const EdgeInsets.only(bottom: 16),
      itemCount: entries.length,
      separatorBuilder: (context, index) => const Divider(height: 1),
      itemBuilder: (context, index) {
        final name = (entries[index]['name'] ?? '').toString();
        return ListTile(
          dense: true,
          contentPadding: EdgeInsets.zero,
          title: Text(name),
        );
      },
    );
  }
}
