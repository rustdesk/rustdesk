import 'dart:convert';
import 'dart:math';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/models/peer_model.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:get/get.dart';

import '../common.dart';
import 'model.dart';

enum PeerTabIndex {
  recent,
  fav,
  lan,
  ab,
  group,
}

class PeerTabModel with ChangeNotifier {
  WeakReference<FFI> parent;
  int get currentTab => _currentTab;
  int _currentTab = 0; // index in tabNames
  List<String> tabNames = [
    'Recent sessions',
    'Favorites',
    'Discovered',
    'Address book',
    'Group',
  ];
  final List<IconData> icons = [
    Icons.access_time_filled,
    Icons.star,
    Icons.explore,
    IconFont.addressBook,
    Icons.group,
  ];
  final List<bool> _isVisible = List.filled(5, true, growable: false);
  List<bool> get isVisible => _isVisible;
  List<int> get indexs => List.generate(tabNames.length, (index) => index);
  List<int> get visibleIndexs => indexs.where((e) => _isVisible[e]).toList();
  List<Peer> _selectedPeers = List.empty(growable: true);
  List<Peer> get selectedPeers => _selectedPeers;
  bool _multiSelectionMode = false;
  bool get multiSelectionMode => _multiSelectionMode;
  List<Peer> _currentTabCachedPeers = List.empty(growable: true);
  List<Peer> get currentTabCachedPeers => _currentTabCachedPeers;
  bool _isShiftDown = false;
  bool get isShiftDown => _isShiftDown;
  String _lastId = '';
  String get lastId => _lastId;

  PeerTabModel(this.parent) {
    // visible
    try {
      final option = bind.getLocalFlutterOption(k: 'peer-tab-visible');
      if (option.isNotEmpty) {
        List<dynamic> decodeList = jsonDecode(option);
        if (decodeList.length == _isVisible.length) {
          for (int i = 0; i < _isVisible.length; i++) {
            if (decodeList[i] is bool) {
              _isVisible[i] = decodeList[i];
            }
          }
        }
      }
    } catch (e) {
      debugPrint("failed to get peer tab visible list:$e");
    }
    // init currentTab
    _currentTab =
        int.tryParse(bind.getLocalFlutterOption(k: 'peer-tab-index')) ?? 0;
    if (_currentTab < 0 || _currentTab >= tabNames.length) {
      _currentTab = 0;
    }
    _trySetCurrentTabToFirstVisible();
  }

  setCurrentTab(int index) {
    if (_currentTab != index) {
      _currentTab = index;
      notifyListeners();
    }
  }

  String tabTooltip(int index) {
    if (index >= 0 && index < tabNames.length) {
      return translate(tabNames[index]);
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

  setMultiSelectionMode(bool mode) {
    _multiSelectionMode = mode;
    if (!mode) {
      _selectedPeers.clear();
      _lastId = '';
    }
    notifyListeners();
  }

  select(Peer peer) {
    if (!_multiSelectionMode) {
      // https://github.com/flutter/flutter/issues/101275#issuecomment-1604541700
      // After onTap, the shift key should be pressed for a while when not in multiselection mode,
      // because onTap is delayed when onDoubleTap is not null
      if (isDesktop && !_isShiftDown) return;
      _multiSelectionMode = true;
    }
    final cached = _currentTabCachedPeers.map((e) => e.id).toList();
    int thisIndex = cached.indexOf(peer.id);
    int lastIndex = cached.indexOf(_lastId);
    if (_isShiftDown && thisIndex >= 0 && lastIndex >= 0) {
      int start = min(thisIndex, lastIndex);
      int end = max(thisIndex, lastIndex);
      bool remove = isPeerSelected(peer.id);
      for (var i = start; i <= end; i++) {
        if (remove) {
          if (isPeerSelected(cached[i])) {
            _selectedPeers.removeWhere((p) => p.id == cached[i]);
          }
        } else {
          if (!isPeerSelected(cached[i])) {
            _selectedPeers.add(_currentTabCachedPeers[i]);
          }
        }
      }
    } else {
      if (isPeerSelected(peer.id)) {
        _selectedPeers.removeWhere((p) => p.id == peer.id);
      } else {
        _selectedPeers.add(peer);
      }
    }
    _lastId = peer.id;
    notifyListeners();
  }

  setCurrentTabCachedPeers(List<Peer> peers) {
    Future.delayed(Duration.zero, () {
      _currentTabCachedPeers = peers;
      notifyListeners();
    });
  }

  selectAll() {
    _selectedPeers = _currentTabCachedPeers.toList();
    notifyListeners();
  }

  bool isPeerSelected(String id) {
    return selectedPeers.firstWhereOrNull((p) => p.id == id) != null;
  }

  setShiftDown(bool v) {
    if (_isShiftDown != v) {
      _isShiftDown = v;
      if (_multiSelectionMode) {
        notifyListeners();
      }
    }
  }

  setTabVisible(int index, bool visible) {
    if (index >= 0 && index < _isVisible.length) {
      if (_isVisible[index] != visible) {
        _isVisible[index] = visible;
        if (index == _currentTab && !visible) {
          _trySetCurrentTabToFirstVisible();
        } else if (visible && visibleIndexs.length == 1) {
          _currentTab = index;
        }
        try {
          bind.setLocalFlutterOption(
              k: 'peer-tab-visible', v: jsonEncode(_isVisible));
        } catch (_) {}
        notifyListeners();
      }
    }
  }

  _trySetCurrentTabToFirstVisible() {
    if (!_isVisible[_currentTab]) {
      int firstVisible = _isVisible.indexWhere((e) => e);
      if (firstVisible >= 0) {
        _currentTab = firstVisible;
      }
    }
  }
}
