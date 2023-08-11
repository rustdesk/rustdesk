import 'dart:async';
import 'package:flutter/material.dart';
import 'package:get/get.dart';

class LoadingDotWidget extends StatefulWidget {
  final int count;
  final double size;
  final int duration;
  LoadingDotWidget(
      {Key? key, required this.size, this.count = 3, this.duration = 200})
      : super(key: key);

  @override
  State<LoadingDotWidget> createState() => _LoadingDotWidgetState();
}

class _LoadingDotWidgetState extends State<LoadingDotWidget> {
  int counter = 0;
  Timer? timer;

  @override
  void initState() {
    super.initState();
    startAnimation();
  }

  @override
  void dispose() {
    timer?.cancel();
    super.dispose();
  }

  void startAnimation() {
    timer = Timer.periodic(Duration(milliseconds: widget.duration), (timer) {
      if (mounted) {
        setState(() {
          counter = (counter + 1) % widget.count;
        });
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    getChild(int index) {
      return AnimatedContainer(
        duration: Duration(milliseconds: widget.duration),
        width: counter == index ? widget.size : widget.size / 2,
        height: counter == index ? widget.size : widget.size / 2,
        decoration: BoxDecoration(
          shape: BoxShape.circle,
          color: Colors.grey,
        ),
      ).marginSymmetric(horizontal: widget.size);
    }

    return Center(
      child: Row(
          mainAxisAlignment: MainAxisAlignment.center,
          children: List.generate(widget.count, (e) => e)
              .map((e) => getChild(e))
              .toList()),
    );
  }
}
