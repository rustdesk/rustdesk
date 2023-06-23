import 'package:flutter/material.dart';
import 'package:flutter_hbb/models/platform_model.dart';

import '../common.dart';
import 'model.dart';

const int groupTabIndex = 4;
const String defaultGroupTabname = 'Group';

class PeerTabModel with ChangeNotifier {
  WeakReference<FFI> parent;
  int get currentTab => _currentTab;
  int _currentTab = 0; // index in tabNames
  List<String> tabNames = [
    'Recent Sessions',
    'Favorites',
    'Discovered',
    'Address Book',
    //defaultGroupTabname,
  ];
  final List<IconData> icons = [
    Icons.access_time_filled,
    Icons.star,
    Icons.explore,
    IconFont.addressBook,
    Icons.group,
  ];
  List<int> get indexs => List.generate(tabNames.length, (index) => index);

  PeerTabModel(this.parent) {
    // init currentTab
    _currentTab =
        int.tryParse(bind.getLocalFlutterConfig(k: 'peer-tab-index')) ?? 0;
    if (_currentTab < 0 || _currentTab >= tabNames.length) {
      _currentTab = 0;
    }
  }

  setCurrentTab(int index) {
    if (_currentTab != index) {
      _currentTab = index;
      notifyListeners();
    }
  }

  String tabTooltip(int index, String groupName) {
    if (index >= 0 && index < tabNames.length) {
      if (index == groupTabIndex) {
        if (gFFI.userModel.isAdmin.value || groupName.isEmpty) {
          return translate(defaultGroupTabname);
        } else {
          return '${translate('Group')}: $groupName';
        }
      } else {
        return translate(tabNames[index]);
      }
    }
    assert(false);
    return index.toString();
  }

  IconData tabIcon(int index) {
    if (index >= 0 && index < tabNames.length) {
      return icons[index];
    }
    assert(false);
    return Icons.help;
  }
}
