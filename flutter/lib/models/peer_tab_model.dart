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
  List<Peer> _selectedPeers = List.empty(growable: true);
  List<Peer> get selectedPeers => _selectedPeers;
  bool get multiSelectionMode => _selectedPeers.isNotEmpty;
  List<Peer> _currentTabCachedPeers = List.empty(growable: true);
  List<Peer> get currentTabCachedPeers => _currentTabCachedPeers;
  bool isShiftDown = false;
  String? _shiftAnchorId;

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
      if (index == PeerTabIndex.group.index) {
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

  togglePeerSelect(Peer peer) {
    final cached = _currentTabCachedPeers.map((e) => e.id).toList();
    int thisIndex = cached.indexOf(peer.id);
    int closestIndex = -1;
    String? closestId;
    int smallestDiff = -1;
    for (var i = 0; i < cached.length; i++) {
      if (isPeerSelected(cached[i])) {
        int diff = (i - thisIndex).abs();
        if (smallestDiff == -1 || diff < smallestDiff) {
          closestIndex = i;
          closestId = cached[i];
          smallestDiff = diff;
        }
      }
    }
    if (isShiftDown &&
        thisIndex >= 0 &&
        closestIndex >= 0 &&
        closestId != null) {
      int shiftAnchorIndex = cached.indexOf(_shiftAnchorId ?? '');
      if (shiftAnchorIndex < 0) {
        // use closest as shift anchor, rather than focused which we don't have
        shiftAnchorIndex = closestIndex;
        _shiftAnchorId = closestId;
      }
      int start = min(shiftAnchorIndex, thisIndex);
      int end = max(shiftAnchorIndex, thisIndex);
      _selectedPeers.clear();
      for (var i = start; i <= end; i++) {
        if (!isPeerSelected(cached[i])) {
          _selectedPeers.add(_currentTabCachedPeers[i]);
        }
      }
    } else {
      if (isPeerSelected(peer.id)) {
        _selectedPeers.removeWhere((p) => p.id == peer.id);
      } else {
        _selectedPeers.add(peer);
      }
      _shiftAnchorId = null;
    }
    notifyListeners();
  }

  closeSelection() {
    _selectedPeers.clear();
    _shiftAnchorId = null;
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
}
