import 'dart:io';
import 'package:flutter/material.dart';
import 'package:get/get.dart';
import 'package:flutter_svg/flutter_svg.dart';
import 'package:flutter_hbb/models/platform_model.dart';

import '../../common.dart';

typedef KBChoosedCallback = bool Function(String);

const double _kImageMarginVertical = 6.0;
const double _kImageMarginHorizental = 10.0;
const double _kImageBoarderWidth = 4.0;
const double _kImagePaddingWidth = 4.0;
const Color _kImageBorderColor = Color.fromARGB(125, 202, 247, 2);
const double _kBorderRadius = 6.0;
const String _kKBLayoutTypeISO = 'ISO';
const String _kKBLayoutTypeNotISO = 'Not ISO';

const _kKBLayoutImageMap = {
  _kKBLayoutTypeISO: 'KB_LAYOUT_ISO',
  _kKBLayoutTypeNotISO: 'KB_LAYOUT_NOT_ISO',
};

class _KBImage extends StatelessWidget {
  final String kbLayoutType;
  final double imageWidth;
  final RxString choosedType;
  const _KBImage({
    Key? key,
    required this.kbLayoutType,
    required this.imageWidth,
    required this.choosedType,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return Obx(() {
      return Container(
        decoration: BoxDecoration(
          borderRadius: BorderRadius.circular(_kBorderRadius),
          border: Border.all(
            color: choosedType.value == kbLayoutType
                ? _kImageBorderColor
                : Colors.transparent,
            width: _kImageBoarderWidth,
          ),
        ),
        margin: EdgeInsets.symmetric(
          horizontal: _kImageMarginHorizental,
          vertical: _kImageMarginVertical,
        ),
        padding: EdgeInsets.all(_kImagePaddingWidth),
        child: SvgPicture.asset(
          'assets/${_kKBLayoutImageMap[kbLayoutType] ?? ""}.svg',
          width: imageWidth -
              _kImageMarginHorizental * 2 -
              _kImagePaddingWidth * 2 -
              _kImageBoarderWidth * 2,
        ),
      );
    });
  }
}

class _KBChooser extends StatelessWidget {
  final String kbLayoutType;
  final double imageWidth;
  final RxString choosedType;
  final KBChoosedCallback cb;
  const _KBChooser({
    Key? key,
    required this.kbLayoutType,
    required this.imageWidth,
    required this.choosedType,
    required this.cb,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        TextButton(
          onPressed: () {
            choosedType.value = kbLayoutType;
          },
          child: _KBImage(
            kbLayoutType: kbLayoutType,
            imageWidth: imageWidth,
            choosedType: choosedType,
          ),
          style: TextButton.styleFrom(padding: EdgeInsets.zero),
        ),
        TextButton(
          child: Row(
            children: [
              Obx(() => Radio(
                    splashRadius: 0,
                    value: kbLayoutType,
                    groupValue: choosedType.value,
                    onChanged: (String? newValue) {
                      if (newValue != null) {
                        if (cb(newValue)) {
                          choosedType.value = newValue;
                        }
                      }
                    },
                  )),
              Text(kbLayoutType),
            ],
          ),
          onPressed: () {
            if (cb(kbLayoutType)) {
              choosedType.value = kbLayoutType;
            }
          },
        ),
      ],
    );
  }
}

class KBLayoutTypeChooser extends StatelessWidget {
  final RxString choosedType;
  final double width;
  final double height;
  final double dividerWidth;
  final KBChoosedCallback cb;
  KBLayoutTypeChooser({
    Key? key,
    required this.choosedType,
    required this.width,
    required this.height,
    required this.dividerWidth,
    required this.cb,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    final imageWidth = width / 2 - dividerWidth;
    return Container(
      color: Colors.white,
      child: SizedBox(
        width: width,
        height: height,
        child: Center(
          child: Row(
            children: [
              _KBChooser(
                kbLayoutType: _kKBLayoutTypeISO,
                imageWidth: imageWidth,
                choosedType: choosedType,
                cb: cb,
              ),
              VerticalDivider(
                width: dividerWidth * 2,
              ),
              _KBChooser(
                kbLayoutType: _kKBLayoutTypeNotISO,
                imageWidth: imageWidth,
                choosedType: choosedType,
                cb: cb,
              ),
            ],
          ),
        ),
      ),
    );
  }
}

RxString KBLayoutType = ''.obs;

String getLocalPlatformForKBLayoutType(String peerPlatform) {
  String localPlatform = '';
  if (peerPlatform != 'Mac OS') {
    return localPlatform;
  }

  if (Platform.isWindows) {
    localPlatform = 'Windows';
  } else if (Platform.isLinux) {
    localPlatform = 'Linux';
  }
  // to-do: web desktop support ?
  return localPlatform;
}

showKBLayoutTypeChooserIfNeeded(
  String peerPlatform,
  OverlayDialogManager dialogManager,
) async {
  final localPlatform = getLocalPlatformForKBLayoutType(peerPlatform);
  if (localPlatform == '') {
    return;
  }
  KBLayoutType.value = bind.getLocalKbLayoutType();
  if (KBLayoutType.value == _kKBLayoutTypeISO ||
      KBLayoutType.value == _kKBLayoutTypeNotISO) {
    return;
  }
  showKBLayoutTypeChooser(localPlatform, dialogManager);
}

showKBLayoutTypeChooser(
  String localPlatform,
  OverlayDialogManager dialogManager,
) {
  dialogManager.show((setState, close) {
    return CustomAlertDialog(
      title:
          Text('${translate('Select local keyboard type')} ($localPlatform)'),
      content: KBLayoutTypeChooser(
          choosedType: KBLayoutType,
          width: 360,
          height: 200,
          dividerWidth: 4.0,
          cb: (String v) {
            bind.setLocalKbLayoutType(kbLayoutType: v);
            KBLayoutType.value = bind.getLocalKbLayoutType();
            return v == KBLayoutType.value;
          }),
      actions: [msgBoxButton(translate('Close'), close)],
      onCancel: close,
    );
  });
}
