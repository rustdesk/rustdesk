import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:get/get.dart';
import 'package:url_launcher/url_launcher.dart';

const _eventKeyUpdateMe = 'update-me';
const _eventKeyUpdateMeReady = 'update-me-ready';
const _githubRateLimitErrorMarker =
    'GitHub API rate limit may have been reached';
// Since this is a rare case,
// we will not add a translation for this user message and will simply show it in English.
const _githubRateLimitUserMessage =
    'The download frequency limit may have been reached. Please try again later.';

void handleUpdate(String releasePageUrl) {
  String downloadUrl = releasePageUrl.replaceAll('tag', 'download');
  String version = downloadUrl.substring(downloadUrl.lastIndexOf('/') + 1);
  final String downloadFile =
      bind.mainGetCommonSync(key: 'download-file-$version');
  if (downloadFile.startsWith('error:')) {
    final error = downloadFile.replaceFirst('error:', '');
    msgBox(gFFI.sessionId, 'custom-nocancel-nook-hasclose', 'Error', error,
        releasePageUrl, gFFI.dialogManager);
    return;
  }
  downloadUrl = '$downloadUrl/$downloadFile';

  SimpleWrapper<String> downloadId = SimpleWrapper('');
  SimpleWrapper<VoidCallback> onCanceled = SimpleWrapper(() {});
  SimpleWrapper<bool> pendingCancel = SimpleWrapper(false);
  SimpleWrapper<bool> cancelInFlight = SimpleWrapper(false);
  SimpleWrapper<Future<void> Function()> cancelDownload =
      SimpleWrapper(() async {});
  gFFI.dialogManager.dismissAll();
  gFFI.dialogManager.show((setState, close, context) {
    cancelDownload.value = () async {
      if (cancelInFlight.value) {
        return;
      }
      final id = downloadId.value;
      if (id.isEmpty) {
        pendingCancel.value = true;
        return;
      }
      cancelInFlight.value = true;
      try {
        pendingCancel.value = false;
        onCanceled.value();
        await bind.mainSetCommon(key: 'cancel-downloader', value: id);
        // Wait for the downloader to be removed.
        for (int i = 0; i < 10; i++) {
          await Future.delayed(const Duration(milliseconds: 300));
          final isCanceled = 'error:Downloader not found' ==
              await bind.mainGetCommon(key: 'download-data-$id');
          if (isCanceled) {
            break;
          }
        }
        close();
      } finally {
        cancelInFlight.value = false;
      }
    };
    return CustomAlertDialog(
        title: Text(translate('Downloading {$appName}')),
        content: UpdateProgress(releasePageUrl, downloadUrl, downloadId,
                onCanceled, pendingCancel, cancelDownload)
            .marginSymmetric(horizontal: 8)
            .paddingOnly(top: 12),
        actions: [
          dialogButton(translate('Cancel'), onPressed: () async {
            await cancelDownload.value();
          }, isOutline: true),
        ]);
  });
}

void _showUpdateError(String releasePageUrl, String error,
    {String messageKey = 'download-new-version-failed-tip',
    bool showRetry = true,
    bool showErrorDetail = true,
    String? userMessage}) {
  debugPrint('Update error: $error');
  final dialogManager = gFFI.dialogManager;
  final visibleError = userMessage ?? (showErrorDetail ? error : null);

  jumplink() {
    launchUrl(Uri.parse(releasePageUrl));
    dialogManager.dismissAll();
  }

  retry() {
    dialogManager.dismissAll();
    handleUpdate(releasePageUrl);
  }

  dialogManager.dismissAll();
  dialogManager.show(
    (setState, close, context) => CustomAlertDialog(
      title: null,
      content: SelectionArea(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            msgboxContent('custom-nocancel-nook-hasclose', 'Error', messageKey),
            if (visibleError != null) ...[
              const SizedBox(height: 8),
              Text(visibleError),
            ],
          ],
        ),
      ),
      actions: [
        dialogButton('Download', onPressed: jumplink),
        if (showRetry) dialogButton('Retry', onPressed: retry),
        dialogButton('Close', onPressed: close),
      ],
    ),
    tag: 'custom-nocancel-nook-hasclose-Error-Error',
  );
}

void _showPreparingForInstallation() {
  gFFI.dialogManager.dismissAll();
  gFFI.dialogManager.show(
    (setState, close, context) => CustomAlertDialog(
      title: Text(translate('Preparing for installation ...')),
      content: const LinearProgressIndicator(
        minHeight: 20,
        borderRadius: BorderRadius.all(Radius.circular(5)),
      ).marginSymmetric(horizontal: 8).paddingOnly(top: 12),
      actions: const [],
    ),
    tag: 'preparing-for-installation',
  );
}

class UpdateProgress extends StatefulWidget {
  final String releasePageUrl;
  final String downloadUrl;
  final SimpleWrapper<String> downloadId;
  final SimpleWrapper<VoidCallback> onCanceled;
  final SimpleWrapper<bool> pendingCancel;
  final SimpleWrapper<Future<void> Function()> cancelDownload;
  UpdateProgress(this.releasePageUrl, this.downloadUrl, this.downloadId,
      this.onCanceled, this.pendingCancel, this.cancelDownload,
      {Key? key})
      : super(key: key);

  @override
  State<UpdateProgress> createState() => UpdateProgressState();
}

class UpdateProgressState extends State<UpdateProgress> {
  Timer? _timer;
  int? _totalSize;
  int _downloadedSize = 0;
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
      if (widget.pendingCancel.value) {
        await widget.cancelDownload.value();
      }
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
    if (error.contains(_githubRateLimitErrorMarker)) {
      _showUpdateError(widget.releasePageUrl, error,
          userMessage: _githubRateLimitUserMessage);
    } else {
      _showUpdateError(widget.releasePageUrl, error, showErrorDetail: false);
    }
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
      if (_totalSize != null && _downloadedSize >= _totalSize!) {
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
    msgBox(
      gFFI.sessionId,
      'custom-nocancel',
      '{$appName} Update',
      '{$appName}-to-update-tip',
      '',
      gFFI.dialogManager,
      onSubmit: () {
        debugPrint('Downloaded, update to new version now');
        if (isMacOS) {
          _showPreparingForInstallation();
          platformFFI.registerEventHandler(
              _eventKeyUpdateMeReady, _eventKeyUpdateMeReady, (evt) async {
            platformFFI.unregisterEventHandler(
                _eventKeyUpdateMeReady, _eventKeyUpdateMeReady);
            gFFI.dialogManager.dismissAll();
          }, replace: true);
        }
        platformFFI.registerEventHandler(_eventKeyUpdateMe, _eventKeyUpdateMe,
            (evt) async {
          platformFFI.unregisterEventHandler(
              _eventKeyUpdateMe, _eventKeyUpdateMe);
          if (isMacOS) {
            platformFFI.unregisterEventHandler(
                _eventKeyUpdateMeReady, _eventKeyUpdateMeReady);
          }
          if (evt.containsKey('error')) {
            _showUpdateError(widget.releasePageUrl, evt['error'] as String,
                showRetry: false, showErrorDetail: false);
          }
        }, replace: true);
        bind.mainSetCommon(key: 'update-me', value: widget.downloadUrl);
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
