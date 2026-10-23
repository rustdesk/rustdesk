import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:get/get.dart';
import 'package:url_launcher/url_launcher.dart';

const _eventKeyUpdateMe = 'update-me';

Future<void> _handleUpdateMe(
    String releasePageUrl, Map<String, dynamic> evt) async {
  platformFFI.unregisterEventHandler(_eventKeyUpdateMe, _eventKeyUpdateMe);
  if (evt.containsKey('error')) {
    _showUpdateError(releasePageUrl, evt['error'] as String);
  }
}

Future<void> handleUpdate(String releasePageUrl) async {
  final dialogClosed = _showVerifyingUpdate();
  final String? downloadUrl;
  try {
    downloadUrl = await waitForUpdateVerification(
      bind.mainGetCommon(key: 'verified-download-url-$releasePageUrl'),
      dialogClosed,
    );
  } catch (error) {
    _showUpdateError(releasePageUrl, error.toString());
    return;
  }
  if (downloadUrl == null) {
    return;
  }
  final verifiedDownloadUrl = downloadUrl;
  if (downloadUrl.startsWith('error:')) {
    _showUpdateError(releasePageUrl, downloadUrl.replaceFirst('error:', ''));
    return;
  }

  SimpleWrapper downloadId = SimpleWrapper('');
  SimpleWrapper<VoidCallback> onCanceled = SimpleWrapper(() {});
  gFFI.dialogManager.dismissAll();
  gFFI.dialogManager.show((setState, close, context) {
    return CustomAlertDialog(
        title: Text(translate('Downloading {$appName}')),
        content: UpdateProgress(
                releasePageUrl, verifiedDownloadUrl, downloadId, onCanceled)
            .marginSymmetric(horizontal: 8)
            .paddingOnly(top: 12),
        actions: [
          dialogButton('Cancel', onPressed: () async {
            onCanceled.value();
            await bind.mainSetCommon(
                key: 'cancel-downloader', value: downloadId.value);
            // Wait for the downloader to be removed.
            for (int i = 0; i < 10; i++) {
              await Future.delayed(const Duration(milliseconds: 300));
              final isCanceled = 'error:Downloader not found' ==
                  await bind.mainGetCommon(
                      key: 'download-data-${downloadId.value}');
              if (isCanceled) {
                break;
              }
            }
            close();
          }, isOutline: true),
        ]);
  });
}

Future<T?> waitForUpdateVerification<T extends Object>(
  Future<T> verification,
  Future<void> dialogClosed,
) async {
  final dialogClosedMarker = Object();
  final result = await Future.any<Object>([
    verification.then<Object>((value) => value),
    dialogClosed.then<Object>((_) => dialogClosedMarker),
  ]);
  return identical(result, dialogClosedMarker) ? null : result as T;
}

Future<void> _showVerifyingUpdate() async {
  gFFI.dialogManager.dismissAll();
  await gFFI.dialogManager.show<void>(
    (setState, close, context) => CustomAlertDialog(
      title: Text(translate('Preparing for installation ...')),
      content: const LinearProgressIndicator(),
      actions: [
        dialogButton('Close', onPressed: close, isOutline: true),
      ],
      onCancel: close,
    ),
    tag: 'verifying-update',
  );
}

void _showUpdateError(String releasePageUrl, String error) {
  debugPrint('Update error: $error');
  gFFI.dialogManager.dismissAll();
  msgBox(gFFI.sessionId, 'custom-nocancel-nook-hasclose', 'Error', error,
      releasePageUrl, gFFI.dialogManager);
}

class UpdateProgress extends StatefulWidget {
  final String releasePageUrl;
  final String downloadUrl;
  final SimpleWrapper downloadId;
  final SimpleWrapper onCanceled;
  UpdateProgress(
      this.releasePageUrl, this.downloadUrl, this.downloadId, this.onCanceled,
      {Key? key})
      : super(key: key);

  @override
  State<UpdateProgress> createState() => UpdateProgressState();
}

class UpdateProgressState extends State<UpdateProgress> {
  Timer? _timer;
  int? _totalSize;
  int _downloadedSize = 0;
  bool _downloadFinished = false;
  int _getDataFailedCount = 0;
  final String _eventKeyDownloadNewVersion = 'download-new-version';

  @override
  void initState() {
    super.initState();
    widget.onCanceled.value = () {
      cancelQueryTimer();
    };
    platformFFI.registerEventHandler(_eventKeyDownloadNewVersion,
        _eventKeyDownloadNewVersion, handleDownloadNewVersion,
        replace: true);
    bind.mainSetCommon(key: 'download-new-version', value: widget.downloadUrl);
  }

  @override
  void dispose() {
    cancelQueryTimer();
    platformFFI.unregisterEventHandler(
        _eventKeyDownloadNewVersion, _eventKeyDownloadNewVersion);
    platformFFI.unregisterEventHandler(_eventKeyUpdateMe, _eventKeyUpdateMe);
    super.dispose();
  }

  void cancelQueryTimer() {
    _timer?.cancel();
    _timer = null;
  }

  Future<void> handleDownloadNewVersion(Map<String, dynamic> evt) async {
    if (evt.containsKey('id')) {
      widget.downloadId.value = evt['id'] as String;
      _timer = Timer.periodic(const Duration(milliseconds: 300), (timer) {
        _updateDownloadData();
      });
    } else {
      if (evt.containsKey('error')) {
        _onError(evt['error'] as String);
      } else {
        // unreachable
        _onError('$evt');
      }
    }
  }

  void _onError(String error) {
    cancelQueryTimer();

    debugPrint('Download new version error: $error');
    final msgBoxType = 'custom-nocancel-nook-hasclose';
    final msgBoxTitle = 'Error';
    final msgBoxText = 'download-new-version-failed-tip';
    final dialogManager = gFFI.dialogManager;
    final releasePageUrl = widget.releasePageUrl;

    close() {
      dialogManager.dismissAll();
    }

    jumplink() {
      launchUrl(Uri.parse(releasePageUrl));
      dialogManager.dismissAll();
    }

    retry() {
      dialogManager.dismissAll();
      handleUpdate(releasePageUrl);
    }

    final List<Widget> buttons = [
      dialogButton('Download', onPressed: jumplink),
      dialogButton('Retry', onPressed: retry),
      dialogButton('Close', onPressed: close),
    ];
    dialogManager.dismissAll();
    dialogManager.show(
      (setState, close, context) => CustomAlertDialog(
        title: null,
        content: SelectionArea(
            child: msgboxContent(msgBoxType, msgBoxTitle, msgBoxText)),
        actions: buttons,
      ),
      tag: '$msgBoxType-$msgBoxTitle-$msgBoxTitle',
    );
  }

  void _updateDownloadData() {
    String err = '';
    String downloadData =
        bind.mainGetCommonSync(key: 'download-data-${widget.downloadId.value}');
    if (downloadData.startsWith('error:')) {
      err = downloadData.substring('error:'.length);
    } else {
      try {
        jsonDecode(downloadData).forEach((key, value) {
          if (key == 'total_size') {
            if (value != null && value is int) {
              _totalSize = value;
            }
          } else if (key == 'downloaded_size') {
            _downloadedSize = value as int;
          } else if (key == 'finished') {
            _downloadFinished = value as bool;
          } else if (key == 'error') {
            if (value != null) {
              err = value.toString();
            }
          }
        });
      } catch (e) {
        _getDataFailedCount += 1;
        debugPrint(
            'Failed to get download data ${widget.downloadUrl}, error $e');
        if (_getDataFailedCount > 3) {
          err = e.toString();
        }
      }
    }
    if (err != '') {
      _onError(err);
    } else {
      if (_downloadFinished &&
          _totalSize != null &&
          _downloadedSize >= _totalSize!) {
        cancelQueryTimer();
        bind.mainSetCommon(
            key: 'remove-downloader', value: widget.downloadId.value);
        if (_totalSize == 0) {
          _onError('The download file size is 0.');
        } else {
          setState(() {});
          updateMsgBox();
        }
      } else {
        setState(() {});
      }
    }
  }

  void updateMsgBox() {
    final releasePageUrl = widget.releasePageUrl;
    final downloadUrl = widget.downloadUrl;
    msgBox(
      gFFI.sessionId,
      'custom-nocancel',
      '{$appName} Update',
      '{$appName}-to-update-tip',
      '',
      gFFI.dialogManager,
      onSubmit: () async {
        debugPrint('Downloaded, update to new version now');
        platformFFI.registerEventHandler(_eventKeyUpdateMe, _eventKeyUpdateMe,
            (evt) => _handleUpdateMe(releasePageUrl, evt),
            replace: true);
        try {
          await bind.mainSetCommon(key: 'update-me', value: downloadUrl);
        } catch (error) {
          platformFFI.unregisterEventHandler(
              _eventKeyUpdateMe, _eventKeyUpdateMe);
          _showUpdateError(releasePageUrl, error.toString());
        }
      },
      submitTimeout: 5,
    );
  }

  @override
  Widget build(BuildContext context) {
    getValue() => _totalSize == null
        ? 0.0
        : (_totalSize == 0 ? 1.0 : _downloadedSize / _totalSize!);
    return LinearProgressIndicator(
      value: getValue(),
      minHeight: 20,
      borderRadius: BorderRadius.circular(5),
      backgroundColor: Colors.grey[300],
      valueColor: const AlwaysStoppedAnimation<Color>(Colors.blue),
    );
  }
}
