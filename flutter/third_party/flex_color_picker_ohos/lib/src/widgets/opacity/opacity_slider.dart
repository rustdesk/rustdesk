// ignore_for_file: use_super_parameters

import 'dart:async';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';

import 'opacity_slider_thumb.dart';
import 'opacity_slider_track.dart';

/// A custom slider used adjust the opacity value of a RGB color.
///
/// The slider has a typical checkered background often used in imaging
/// editing program to show the degree of opacity an image has.
///
/// The RGB [Color] we are adjusting the opacity for, is passed in the
/// [color] property and drawn as opacity gradient over the checkered
/// background to visually indicate how opaque or transparent the current
/// opacity value is. The opacity value is shown as 0 ... 100 (%) on
/// the adjustment thumb, 0 being fully transparent and 100, fully opaque.
///
/// The slider has 255 steps so that it is possible to select any corresponding
/// 8-bit alpha channel value. If the opacity is applied to a color using
/// `withOpacity` and the alpha value displayed in the resulting color, this
/// be observed.
///
/// The opacity value is returned via the onChanged called back. There are
/// also callbacks for [onChangeStart] and [onChangeEnd].
class OpacitySlider extends StatelessWidget {
  /// Create the opacity slider.
  const OpacitySlider({
    Key? key,
    required this.opacity,
    required this.color,
    required this.onChanged,
    this.onChangeStart,
    this.onChangeEnd,
    this.thumbRadius = 16,
    this.trackHeight = 22,
    this.focusNode,
  })  : assert(thumbRadius >= 12 && thumbRadius <= 30,
            'The thumbRadius must be 12 to 30.'),
        assert(trackHeight >= 10 && trackHeight <= 50,
            'The trackHeight must be 10 to 50.'),
        super(key: key);

  /// Current opacity value.
  final double opacity;

  /// Current selected color.
  final Color color;

  /// Called when the opacity value is changed.
  final ValueChanged<double> onChanged;

  /// Called when the user starts selecting a new value for the opacity slider.
  ///
  /// This callback shouldn't be used to update the slider value (use
  /// [onChanged] for that), but rather to be notified when the user has started
  /// selecting a new value by starting a drag or with a tap.
  ///
  /// The value passed will be the last value that the slider had before the
  /// change began.
  final ValueChanged<double>? onChangeStart;

  /// Called when the user is done selecting a new value for the slider.
  ///
  /// This callback shouldn't be used to update the slider value (use
  /// [onChanged] for that), but rather to know when the user has completed
  /// selecting a new value by ending a drag or a click.
  final ValueChanged<double>? onChangeEnd;

  /// The radius of the thumb on the opacity slider.
  ///
  /// Defaults to 16.
  final double thumbRadius;

  /// The height of the slider track.
  ///
  /// Defaults to 36
  final double trackHeight;

  /// An optional focus node to use as the focus node for this widget.
  ///
  /// If one is not supplied, then one will be automatically allocated, owned,
  /// and managed by this widget. The widget will be focusable even if a
  /// focusNode is not supplied. If supplied, the given focusNode will be
  /// hosted by this widget, but not owned. See FocusNode for more information
  /// on what being hosted and/or owned implies.
  ///
  /// Supplying a focus node is sometimes useful if an ancestor to this
  /// widget wants to control when this widget has the focus. The owner will
  /// be responsible for calling FocusNode.dispose on the focus node when it is
  /// done with it, but this widget will attach/detach and reparent the
  /// node when needed.
  final FocusNode? focusNode;

  @override
  Widget build(BuildContext context) {
    return RepaintBoundary(
      child: FutureBuilder<ui.Image>(
        // ignore: discarded_futures
        future: getTrackImage(),
        builder: (BuildContext context, AsyncSnapshot<ui.Image> snapshot) {
          if (!snapshot.hasData) return const SizedBox.shrink();
          return Theme(
            data: _opacitySliderTheme(color, trackHeight, thumbRadius),
            child: Slider(
              value: opacity,
              // If we do 255 divisions we can get a discrete step for each
              // alpha value, even if we only display int 0...100 as opacity.
              divisions: 255,
              onChanged: onChanged,
              onChangeStart: onChangeStart,
              onChangeEnd: onChangeEnd,
              focusNode: focusNode,
            ),
          );
        },
      ),
    );
  }
}

// The background image used on the slider track, it is the typically used
// square background used on transparent images in image editing software.
ui.Image? _trackImage;

/// Load the background grid image for the opacity slider as a dart.ui
/// [Image] from assets.
Future<ui.Image> getTrackImage() {
  if (_trackImage != null) return Future<ui.Image>.value(_trackImage);
  final Completer<ui.Image> completer = Completer<ui.Image>();
  const AssetImage('assets/opacity.png', package: 'flex_color_picker')
      .resolve(ImageConfiguration.empty)
      .addListener(ImageStreamListener((ImageInfo info, bool _) {
    _trackImage = info.image;
    completer.complete(_trackImage);
  }));
  return completer.future;
}

/// To style the opacity slider we use a theme that changes with
/// passed in color value that the slider is manipulating opacity for.
ThemeData _opacitySliderTheme(
    Color color, double trackHeight, double thumbRadius) {
  // The thumbColor used for outline and text on [color] must have good
  // contrast with [color] so we can se it around the thumb and for text
  // written on top of the thumb fill [color].
  final Color thumbTextColor =
      ThemeData.estimateBrightnessForColor(color) == Brightness.light
          ? const Color(0xFF333333)
          : Colors.white;
  return ThemeData.light().copyWith(
    sliderTheme: SliderThemeData(
      trackHeight: trackHeight,
      thumbColor: thumbTextColor,
      // The track uses a custom slider track, with a grid image background.
      trackShape: OpacitySliderTrack(
        color: color,
        thumbRadius: thumbRadius,
        image: _trackImage!,
      ),
      // The thumb uses a custom thumb that is hollow.
      thumbShape: OpacitySliderThumb(
        color: color,
        enabledThumbRadius: thumbRadius,
      ),
    ),
  );
}
