import 'dart:async';
import 'dart:convert';
import 'dart:typed_data';

import 'package:zmodem/zmodem.dart';

export 'package:zmodem/zmodem.dart' show ZModemFileInfo;

typedef ZModemInputHandler = void Function(String output);

typedef ZModemOfferHandler = void Function(ZModemOffer offer);

typedef ZModemRequestHandler = Future<Iterable<ZModemOffer>> Function();

abstract class ZModemOffer {
  ZModemFileInfo get info;

  Stream<Uint8List> accept(int offset);

  void skip();
}

class ZModemCallbackOffer implements ZModemOffer {
  @override
  final ZModemFileInfo info;

  final Stream<Uint8List> Function(int offset) onAccept;

  final void Function()? onSkip;

  ZModemCallbackOffer(this.info, {required this.onAccept, this.onSkip});

  @override
  Stream<Uint8List> accept(int offset) {
    return onAccept(offset);
  }

  @override
  void skip() {
    onSkip?.call();
  }
}

final _zmodemSenderInit = '**\x18B0000000'.codeUnits;

final _zmodemReceiverInit = '**\x18B0100000'.codeUnits;

class ZModemMux {
  /// Data from the underlying data channel.
  final Stream<Uint8List> stdout;

  /// The sink to write data to the underlying data channel.
  final StreamSink<List<int>> stdin;

  /// The callback to receive data that should be written to the terminal.
  ZModemInputHandler? onTerminalInput;

  /// The callback to handle file receiving. If not set, all offers will be
  /// skipped.
  ZModemOfferHandler? onFileOffer;

  /// The callback to handle file sending. If not set, all requests will be
  /// ignored.
  ZModemRequestHandler? onFileRequest;

  ZModemMux({required this.stdin, required this.stdout}) {
    _stdoutSubscription = stdout.listen(_handleStdout);
  }

  /// Subscriptions to [stdout]. Used to pause/resume the stream when no more
  /// space is available in local buffers.
  late final StreamSubscription<Uint8List> _stdoutSubscription;

  late final _terminalSink = StreamController<List<int>>(
      // onPause: _stdoutSubscription.pause,
      // onResume: _stdoutSubscription.resume,
      )
    ..stream
        .transform(Utf8Decoder(allowMalformed: true))
        .listen(onTerminalInput);

  /// Current ZModem session. If null, no session is active.
  ZModemCore? _session;

  /// The sink to write data when receiving a file. If null, no file is being
  /// received.
  StreamController<Uint8List>? _receiveSink;

  /// Offers to send to the remote peer. If null, no offers are being sent.
  Iterator<ZModemOffer>? _fileOffers;

  /// Writes terminal output to the underlying connection. [input] may be
  /// buffered if a ZModem session is active.
  void terminalWrite(String input) {
    if (_session == null) {
      stdin.add(utf8.encode(input));
    }
  }

  /// This is the entry point of multiplexing, dispatching data to ZModem or
  /// terminal depending on the current state.
  void _handleStdout(Uint8List chunk) {
    if (_session != null) {
      _handleZModem(chunk);
      return;
    }

    if (_detectZModem(chunk)) {
      return;
    }

    _terminalSink.add(chunk);
  }

  /// Detects a ZModem session in [chunk] and starts it if found. Returns true
  /// if a session was started.
  bool _detectZModem(Uint8List chunk) {
    final index = chunk.listIndexOf(_zmodemSenderInit) ??
        chunk.listIndexOf(_zmodemReceiverInit);

    if (index != null) {
      _terminalSink.add(Uint8List.sublistView(chunk, 0, index));

      _session = ZModemCore(
        onPlainText: (text) {
          _terminalSink.add([text]);
        },
      );

      _handleZModem(Uint8List.sublistView(chunk, index));
      return true;
    }

    return false;
  }

  void _handleZModem(Uint8List chunk) async {
    for (final event in _session!.receive(chunk)) {
      /// remote is sz
      if (event is ZFileOfferedEvent) {
        _handleZFileOfferedEvent(event);
      } else if (event is ZFileDataEvent) {
        _handleZFileDataEvent(event);
      } else if (event is ZFileEndEvent) {
        await _handleZFileEndEvent(event);
      } else if (event is ZSessionFinishedEvent) {
        await _handleZSessionFinishedEvent(event);
      }

      /// remote is rz
      else if (event is ZReadyToSendEvent) {
        await _handleFileRequestEvent(event);
      } else if (event is ZFileAcceptedEvent) {
        await _handleFileAcceptedEvent(event);
      } else if (event is ZFileSkippedEvent) {
        _handleFileSkippedEvent(event);
      }

      _flush();
    }

    _flush();
  }

  void _handleZFileOfferedEvent(ZFileOfferedEvent event) {
    final onFileOffer = this.onFileOffer;

    if (onFileOffer == null) {
      _session!.skipFile();
      return;
    }

    onFileOffer(_createRemoteOffer(event.fileInfo));
  }

  void _handleZFileDataEvent(ZFileDataEvent event) {
    _receiveSink!.add(event.data as Uint8List);
  }

  Future<void> _handleZFileEndEvent(ZFileEndEvent event) async {
    await _closeReceiveSink();
  }

  Future<void> _handleZSessionFinishedEvent(ZSessionFinishedEvent event) async {
    _flush();
    await _reset();
  }

  Future<void> _handleFileRequestEvent(ZReadyToSendEvent event) async {
    _fileOffers ??= (await onFileRequest?.call())?.iterator;

    _moveToNextOffer();
  }

  Future<void> _handleFileAcceptedEvent(ZFileAcceptedEvent event) async {
    final data = _fileOffers!.current.accept(event.offset);
    var bytesSent = 0;

    await stdin.addStream(
      data.transform(
        StreamTransformer<Uint8List, Uint8List>.fromHandlers(
          handleData: (chunk, sink) {
            bytesSent += chunk.length;
            _session!.sendFileData(chunk);
            sink.add(_session!.dataToSend());
          },
        ),
      ),
    );

    _session!.finishSending(event.offset + bytesSent);
  }

  void _handleFileSkippedEvent(ZFileSkippedEvent event) {
    _fileOffers!.current.skip();
    _moveToNextOffer();
  }

  /// Sends next file offer if available, or closes the session if not.
  void _moveToNextOffer() {
    if (_fileOffers?.moveNext() != true) {
      _closeSession();
      return;
    }

    _session!.offerFile(_fileOffers!.current.info);
  }

  /// Creates a [ZModemOffer] ƒrom the info from remote peer that can be used
  /// by local client to accept or skip the file.
  ZModemOffer _createRemoteOffer(ZModemFileInfo fileInfo) {
    return ZModemCallbackOffer(
      fileInfo,
      onAccept: (offset) {
        _session!.acceptFile(offset);
        _flush();

        _createReceiveSink();
        return _receiveSink!.stream;
      },
      onSkip: () {
        _session!.skipFile();
        _flush();
      },
    );
  }

  void _createReceiveSink() {
    _receiveSink = StreamController<Uint8List>(
      onPause: () {
        // _stdoutSubscription.pause();
      },
      onResume: () {
        // _stdoutSubscription.resume();
      },
    );
  }

  Future<void> _closeReceiveSink() async {
    _stdoutSubscription.resume();
    await _receiveSink?.close();
    _receiveSink = null;
  }

  /// Requests remote to close the session.
  void _closeSession() {
    _session!.finishSession();
  }

  /// Clears all ZModem state.
  Future<void> _reset() async {
    await _closeReceiveSink();
    _fileOffers = null;
    _session = null;
  }

  /// Sends all pending data packets to the remote. No data is automatically
  /// sent to the remote without calling this method.
  void _flush() {
    final dataToSend = _session?.dataToSend();
    if (dataToSend != null && dataToSend.isNotEmpty) {
      stdin.add(dataToSend);
    }
  }
}

extension ListExtension on List<int> {
  String dump() {
    return map((e) => e.toRadixString(16).padLeft(2, '0')).join(' ');
  }

  int? listIndexOf(List<int> other, [int start = 0]) {
    if (other.length + start > length) {
      return null;
    }
    for (var i = start; i < length - other.length; i++) {
      if (this[i] == other[0]) {
        var found = true;
        for (var j = 1; j < other.length; j++) {
          if (this[i + j] != other[j]) {
            found = false;
            break;
          }
        }
        if (found) {
          return i;
        }
      }
    }
    return null;
  }
}
