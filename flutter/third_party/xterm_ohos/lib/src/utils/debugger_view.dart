import 'package:flutter/material.dart';
import 'package:xterm/src/utils/debugger.dart';

class TerminalDebuggerView extends StatefulWidget {
  const TerminalDebuggerView(
    this.debugger, {
    super.key,
    this.scrollController,
    this.onSeek,
  });

  final TerminalDebugger debugger;

  final ScrollController? scrollController;

  final void Function(int?)? onSeek;

  @override
  State<TerminalDebuggerView> createState() => _TerminalDebuggerViewState();
}

class _TerminalDebuggerViewState extends State<TerminalDebuggerView> {
  int? selectedCommand;

  @override
  void initState() {
    widget.debugger.addListener(_onDebuggerChanged);
    super.initState();
  }

  @override
  void didUpdateWidget(covariant TerminalDebuggerView oldWidget) {
    if (oldWidget.debugger != widget.debugger) {
      oldWidget.debugger.removeListener(_onDebuggerChanged);
      widget.debugger.addListener(_onDebuggerChanged);
    }
    super.didUpdateWidget(oldWidget);
  }

  @override
  void dispose() {
    widget.debugger.removeListener(_onDebuggerChanged);
    super.dispose();
  }

  void _onDebuggerChanged() {
    setState(() {});
  }

  @override
  Widget build(BuildContext context) {
    final commands = widget.debugger.commands;
    return ListView.builder(
      itemExtent: 20,
      controller: widget.scrollController,
      itemCount: commands.length,
      itemBuilder: (context, index) {
        final command = commands[index];
        return _CommandItem(
          index,
          command,
          selected: selectedCommand == index,
          onTap: () {
            if (selectedCommand == index) {
              selectedCommand = null;
            } else {
              setState(() => selectedCommand = index);
            }
            widget.onSeek?.call(selectedCommand);
          },
        );
      },
    );
  }
}

class _CommandItem extends StatelessWidget {
  const _CommandItem(
    this.index,
    this.command, {
    this.onTap,
    this.selected = false,
  });

  final int index;

  final TerminalCommand command;

  final bool selected;

  final void Function()? onTap;

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      onTap: onTap,
      child: MouseRegion(
        cursor: SystemMouseCursors.click,
        onEnter: (event) {
          if (event.down) {
            onTap?.call();
          }
        },
        child: Container(
          decoration: BoxDecoration(
            border: Border.all(
              color: selected ? Colors.blue : Colors.transparent,
              width: 2,
            ),
            borderRadius: BorderRadius.circular(5),
          ),
          child: Row(
            children: [
              Container(
                width: 50,
                child: Text(
                  '${index + 1}',
                  style: TextStyle(
                    color: selected ? Colors.blue : Colors.black,
                    fontSize: 14,
                    fontFamily: 'monospace',
                    fontFamilyFallback: [
                      'Menlo',
                      'Monaco',
                      'Consolas',
                      'Liberation Mono',
                      'Courier New',
                      'Noto Sans Mono CJK SC',
                      'Noto Sans Mono CJK TC',
                      'Noto Sans Mono CJK KR',
                      'Noto Sans Mono CJK JP',
                      'Noto Sans Mono CJK HK',
                      'Noto Color Emoji',
                      'Noto Sans Symbols',
                      'monospace',
                      'sans-serif',
                    ],
                  ),
                  textAlign: TextAlign.right,
                ),
              ),
              SizedBox(width: 20),
              Container(
                width: 400,
                child: Text(
                  command.escapedChars,
                  style: TextStyle(
                    color: command.error ? Colors.red : null,
                    fontSize: 14,
                  ),
                ),
              ),
              Expanded(
                child: Container(
                  child: Text(
                    command.explanation.join(','),
                    style: TextStyle(
                      color: command.error ? Colors.red : null,
                      fontSize: 14,
                    ),
                  ),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
