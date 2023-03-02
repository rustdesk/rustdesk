import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:get/get.dart';
import 'package:scroll_pos/scroll_pos.dart';

import '../common.dart';
import 'model.dart';

const int groupTabIndex = 4;
const String defaultGroupTabname = 'Group';

class PeerTabModel with ChangeNotifier {
  WeakReference<FFI> parent;
  int get currentTab => _currentTab;
  int _currentTab = 0; // index in tabNames
  List<int> get visibleOrderedTabs => _visibleOrderedTabs;
  List<int> _visibleOrderedTabs = List.empty(growable: true);
  List<int> get tabOrder => _tabOrder;
  List<int> _tabOrder = List.from([0, 1, 2, 3, 4]); // constant length
  int get tabHiddenFlag => _tabHiddenFlag;
  int _tabHiddenFlag = 0;
  bool get showScrollBtn => _showScrollBtn;
  bool _showScrollBtn = false;
  final List<bool> _fullyVisible = List.filled(5, false);
  bool get leftFullyVisible => _leftFullyVisible;
  bool _leftFullyVisible = false;
  bool get rightFullyVisible => _rightFullyVisible;
  bool _rightFullyVisible = false;
  ScrollPosController sc = ScrollPosController();
  List<String> tabNames = [
    'Recent Sessions',
    'Favorites',
    'Discovered',
    'Address Book',
    defaultGroupTabname,
  ];

  PeerTabModel(this.parent) {
    // init tabHiddenFlag
    _tabHiddenFlag = int.tryParse(
            bind.getLocalFlutterConfig(k: 'hidden-peer-card'),
            radix: 2) ??
        0;
    var tabs = _notHiddenTabs();
    // remove dynamic tabs
    tabs.remove(groupTabIndex);
    // init tabOrder
    try {
      final conf = bind.getLocalFlutterConfig(k: 'peer-tab-order');
      if (conf.isNotEmpty) {
        final json = jsonDecode(conf);
        if (json is List) {
          final List<int> list =
              json.map((e) => int.tryParse(e.toString()) ?? -1).toList();
          if (list.length == _tabOrder.length &&
              _tabOrder.every((e) => list.contains(e))) {
            _tabOrder = list;
          }
        }
      }
    } catch (e) {
      debugPrintStack(label: '$e');
    }
    // init visibleOrderedTabs
    var tempList = _tabOrder.toList();
    tempList.removeWhere((e) => !tabs.contains(e));
    _visibleOrderedTabs = tempList;
    // init currentTab
    _currentTab =
        int.tryParse(bind.getLocalFlutterConfig(k: 'peer-tab-index')) ?? 0;
    if (!tabs.contains(_currentTab)) {
      if (tabs.isNotEmpty) {
        _currentTab = tabs[0];
      } else {
        _currentTab = 0;
      }
    }
    sc.itemCount = _visibleOrderedTabs.length;
  }

  check_dynamic_tabs() {
    var visible = visibleTabs();
    _visibleOrderedTabs = _tabOrder.where((e) => visible.contains(e)).toList();
    if (_visibleOrderedTabs.contains(groupTabIndex) &&
        int.tryParse(bind.getLocalFlutterConfig(k: 'peer-tab-index')) ==
            groupTabIndex) {
      _currentTab = groupTabIndex;
    }
    if (gFFI.userModel.isAdmin.isFalse && gFFI.userModel.groupName.isNotEmpty) {
      tabNames[groupTabIndex] = gFFI.userModel.groupName.value;
    } else {
      tabNames[groupTabIndex] = defaultGroupTabname;
    }
    sc.itemCount = _visibleOrderedTabs.length;
    notifyListeners();
  }

  setCurrentTab(int index) {
    if (_currentTab != index) {
      _currentTab = index;
      notifyListeners();
    }
  }

  setTabFullyVisible(int index, bool visible) {
    if (index >= 0 && index < _fullyVisible.length) {
      if (visible != _fullyVisible[index]) {
        _fullyVisible[index] = visible;
        bool changed = false;
        bool show = _visibleOrderedTabs.any((e) => !_fullyVisible[e]);
        if (show != _showScrollBtn) {
          _showScrollBtn = show;
          changed = true;
        }
        if (_visibleOrderedTabs.isNotEmpty && _visibleOrderedTabs[0] == index) {
          if (_leftFullyVisible != visible) {
            _leftFullyVisible = visible;
            changed = true;
          }
        }
        if (_visibleOrderedTabs.isNotEmpty &&
            _visibleOrderedTabs.last == index) {
          if (_rightFullyVisible != visible) {
            _rightFullyVisible = visible;
            changed = true;
          }
        }
        if (changed) {
          notifyListeners();
        }
      }
    }
  }

  onReorder(oldIndex, newIndex) {
    if (oldIndex < newIndex) {
      newIndex -= 1;
    }
    var list = _visibleOrderedTabs.toList();
    final int item = list.removeAt(oldIndex);
    list.insert(newIndex, item);
    _visibleOrderedTabs = list;

    var tmpTabOrder = _visibleOrderedTabs.toList();
    var left = _tabOrder.where((e) => !tmpTabOrder.contains(e)).toList();
    for (var t in left) {
      _addTabInOrder(tmpTabOrder, t);
    }
    _tabOrder = tmpTabOrder;
    bind.setLocalFlutterConfig(k: 'peer-tab-order', v: jsonEncode(tmpTabOrder));
    notifyListeners();
  }

  onHideShow(int index, bool show) async {
    int bitMask = 1 << index;
    if (show) {
      _tabHiddenFlag &= ~bitMask;
    } else {
      _tabHiddenFlag |= bitMask;
    }
    await bind.setLocalFlutterConfig(
        k: 'hidden-peer-card', v: _tabHiddenFlag.toRadixString(2));
    var visible = visibleTabs();
    _visibleOrderedTabs = _tabOrder.where((e) => visible.contains(e)).toList();
    if (_visibleOrderedTabs.isNotEmpty &&
        !_visibleOrderedTabs.contains(_currentTab)) {
      _currentTab = _visibleOrderedTabs[0];
    }
    notifyListeners();
  }

  List<int> orderedNotFilteredTabs() {
    var list = tabOrder.toList();
    if (_filterGroupCard()) {
      list.remove(groupTabIndex);
    }
    return list;
  }

  // return index array of tabNames
  List<int> visibleTabs() {
    var v = List<int>.empty(growable: true);
    for (int i = 0; i < tabNames.length; i++) {
      if (!_isTabHidden(i) && !_isTabFilter(i)) {
        v.add(i);
      }
    }
    return v;
  }

  String translatedTabname(int index) {
    if (index >= 0 && index < tabNames.length) {
      final name = tabNames[index];
      if (index == groupTabIndex) {
        if (name == defaultGroupTabname) {
          return translate(name);
        } else {
          return name;
        }
      } else {
        return translate(name);
      }
    }
    assert(false);
    return index.toString();
  }

  bool _isTabHidden(int tabindex) {
    return _tabHiddenFlag & (1 << tabindex) != 0;
  }

  bool _isTabFilter(int tabIndex) {
    if (tabIndex == groupTabIndex) {
      return _filterGroupCard();
    }
    return false;
  }

  // return true if hide group card
  bool _filterGroupCard() {
    if (gFFI.groupModel.users.isEmpty ||
        (gFFI.userModel.isAdmin.isFalse && gFFI.userModel.groupName.isEmpty)) {
      return true;
    } else {
      return false;
    }
  }

  List<int> _notHiddenTabs() {
    var v = List<int>.empty(growable: true);
    for (int i = 0; i < tabNames.length; i++) {
      if (!_isTabHidden(i)) {
        v.add(i);
      }
    }
    return v;
  }

  // add tabIndex to list
  _addTabInOrder(List<int> list, int tabIndex) {
    if (!_tabOrder.contains(tabIndex) || list.contains(tabIndex)) {
      return;
    }
    bool sameOrder = true;
    int lastIndex = -1;
    for (int i = 0; i < list.length; i++) {
      var index = _tabOrder.lastIndexOf(list[i]);
      if (index > lastIndex) {
        lastIndex = index;
        continue;
      } else {
        sameOrder = false;
        break;
      }
    }
    if (sameOrder) {
      var indexInTabOrder = _tabOrder.indexOf(tabIndex);
      var left = List.empty(growable: true);
      for (int i = 0; i < indexInTabOrder; i++) {
        left.add(_tabOrder[i]);
      }
      int insertIndex = list.lastIndexWhere((e) => left.contains(e));
      if (insertIndex < 0) {
        insertIndex = 0;
      } else {
        insertIndex += 1;
      }
      list.insert(insertIndex, tabIndex);
    } else {
      list.add(tabIndex);
    }
  }
}
