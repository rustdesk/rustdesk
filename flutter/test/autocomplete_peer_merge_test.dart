import 'package:flutter_hbb/common/widgets/autocomplete.dart';
import 'package:flutter_hbb/models/peer_model.dart';
import 'package:flutter_test/flutter_test.dart';

Peer _peer({
  required String id,
  String alias = '',
  String username = '',
  String hostname = '',
  bool online = false,
}) {
  final peer = Peer(
    id: id,
    username: username,
    hostname: hostname,
    alias: alias,
    platform: '',
    tags: [],
    hash: '',
    password: '',
    forceAlwaysRelay: false,
    rdpPort: '',
    rdpUsername: '',
    loginName: '',
    device_group_name: '',
    note: '',
  );
  peer.online = online;
  return peer;
}

void main() {
  test('merged autocomplete peers keep address book metadata and online state',
      () {
    final peers = mergeAutocompletePeers(
      addressBookPeers: [
        _peer(id: '123456789', alias: 'Office PC', username: 'ab-user'),
      ],
      lanPeers: [
        _peer(id: '123456789', username: 'lan-user', online: true),
      ],
    );

    expect(peers, hasLength(1));
    expect(peers.single.id, '123456789');
    expect(peers.single.alias, 'Office PC');
    expect(peers.single.username, 'ab-user');
    expect(peers.single.online, isTrue);
  });

  test('peer copies preserve online state', () {
    final peer = _peer(id: '987654321', online: true);

    expect(Peer.copy(peer).online, isTrue);
  });

  test('online callbacks update autocomplete-only peers', () {
    final peers = mergeAutocompletePeers(restRecentPeerIds: ['112233445']);

    final changed = updateAutocompletePeerOnlineStates(
      peers,
      onlines: {'112233445'},
      offlines: {},
    );

    expect(changed, isTrue);
    expect(peers.single.online, isTrue);
  });

  test('online query ids are deduplicated and limited', () {
    final peers = List.generate(
      25,
      (index) => _peer(id: index.toString()),
    )..insert(1, _peer(id: '0'));

    final ids = autocompleteOnlineQueryIds(peers, limit: 20);

    expect(ids, hasLength(20));
    expect(ids.first, '0');
    expect(ids.where((id) => id == '0'), hasLength(1));
    expect(ids.last, '19');
  });

  test('empty online query ids cancel pending debounce', () async {
    final queriedIds = <List<String>>[];
    final loader = AllPeersLoader(
      queryOnlines: (ids) async {
        queriedIds.add(ids);
      },
      queryOnlineDebounce: Duration(milliseconds: 1),
    );

    loader.queryOnlines([_peer(id: '123456789')]);
    loader.queryOnlines([]);
    await Future.delayed(Duration(milliseconds: 2));

    expect(queriedIds, isEmpty);
  });

  test('failed online query enqueue does not suppress retry', () async {
    var queryCount = 0;
    final loader = AllPeersLoader(
      queryOnlines: (ids) {
        queryCount += 1;
        return Future<void>.error(Exception('queue full'));
      },
      queryOnlineDebounce: Duration(milliseconds: 1),
    );

    loader.queryOnlines([_peer(id: '123456789')]);
    await Future.delayed(Duration(milliseconds: 2));

    loader.queryOnlines([_peer(id: '123456789')]);
    await Future.delayed(Duration(milliseconds: 2));

    expect(queryCount, 2);
  });

  test('online callback updates currently displayed options', () async {
    final loader = AllPeersLoader(
      queryOnlines: (ids) async {},
      queryOnlineDebounce: Duration(milliseconds: 1),
    );
    final displayedOptions = [_peer(id: '123456789')];

    loader.queryOnlines(displayedOptions);
    loader.updateOnlineStateForTesting({
      'onlines': '123456789',
      'offlines': '',
    });

    expect(displayedOptions.single.online, isTrue);
    await Future.delayed(Duration(milliseconds: 2));
  });

  test('cached online callback state is reapplied after peers merge', () {
    final loader = AllPeersLoader();
    loader.updateOnlineStateForTesting({
      'onlines': '123456789',
      'offlines': '',
    });

    final mergedPeers = [_peer(id: '123456789')];
    loader.applyLastOnlineStateForTesting(mergedPeers);

    expect(mergedPeers.single.online, isTrue);
  });
}
