import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:get/get.dart';

import '../../desktop/widgets/popup_menu.dart';

Future<void> showNativePopupMenu<T>({
  required BuildContext context,
  required RelativeRect position,
  required List<MenuEntryBase<T>> items,
}) {
  final themes =
      InheritedTheme.capture(from: context, to: Navigator.of(context).context);
  return showGeneralDialog<void>(
    context: context,
    useRootNavigator: false,
    barrierDismissible: true,
    barrierLabel: MaterialLocalizations.of(context).modalBarrierDismissLabel,
    barrierColor: Colors.transparent,
    transitionDuration: Duration.zero,
    pageBuilder: (context, _, __) =>
        themes.wrap(_NativePopupMenu<T>(position: position, items: items)),
  );
}

class _NativePopupMenu<T> extends StatefulWidget {
  final RelativeRect position;
  final List<MenuEntryBase<T>> items;

  const _NativePopupMenu({required this.position, required this.items});

  @override
  State<_NativePopupMenu<T>> createState() => _NativePopupMenuState<T>();
}

class _NativePopupMenuState<T> extends State<_NativePopupMenu<T>> {
  final _controller = MenuController();

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) _controller.open();
    });
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final popupShape = theme.popupMenuTheme.shape;
    final shape = popupShape is OutlinedBorder
        ? popupShape
        : const RoundedRectangleBorder(
            borderRadius: BorderRadius.all(Radius.circular(4)));
    return MenuTheme(
      data: MenuThemeData(
        style: MenuStyle(
          backgroundColor: MaterialStatePropertyAll(
              theme.popupMenuTheme.color ?? theme.cardColor),
          surfaceTintColor: const MaterialStatePropertyAll(Colors.transparent),
          elevation: const MaterialStatePropertyAll(8),
          visualDensity: VisualDensity.standard,
          minimumSize: const MaterialStatePropertyAll(Size(112, 0)),
          padding:
              const MaterialStatePropertyAll(EdgeInsets.symmetric(vertical: 8)),
          shape: MaterialStatePropertyAll(shape),
          side: MaterialStatePropertyAll(shape.side),
        ),
      ),
      child: Stack(children: [
        Positioned(
          left: widget.position.left,
          top: widget.position.top,
          child: MenuAnchor(
            controller: _controller,
            onClose: () {
              // MenuAnchor can close during a rebuild after a viewport change.
              WidgetsBinding.instance.addPostFrameCallback((_) {
                if (mounted && ModalRoute.of(context)?.isCurrent == true) {
                  Navigator.pop(context);
                }
              });
            },
            menuChildren: widget.items.map(_buildItem).toList(),
            child: const SizedBox.shrink(),
          ),
        ),
      ]),
    );
  }

  ButtonStyle _buttonStyle(EdgeInsets? padding) {
    final theme = Theme.of(context);
    final buttonTheme = TextButtonTheme.of(context).style;
    final density = buttonTheme?.visualDensity ?? theme.visualDensity;
    final adjustment = density.baseSizeAdjustment;
    final fontSize = theme.textTheme.labelLarge?.fontSize ?? 14;
    final defaultPadding = ButtonStyleButton.scaledPadding(
      theme.useMaterial3
          ? const EdgeInsets.symmetric(horizontal: 12, vertical: 8)
          : const EdgeInsets.all(8),
      const EdgeInsets.symmetric(horizontal: 8),
      const EdgeInsets.symmetric(horizontal: 4),
      MediaQuery.textScalerOf(context).scale(fontSize) / 14,
    );
    final buttonPadding = (buttonTheme?.padding?.resolve({}) ?? defaultPadding)
        .resolve(Directionality.of(context))
        .add(EdgeInsets.symmetric(
            horizontal: math.max(0, adjustment.dx), vertical: adjustment.dy))
        .clamp(EdgeInsets.zero, EdgeInsetsGeometry.infinity);
    final minimumSize = buttonTheme?.minimumSize?.resolve({}) ??
        Size(64, theme.useMaterial3 ? 40 : 36);
    final padded =
        (buttonTheme?.tapTargetSize ?? theme.materialTapTargetSize) ==
            MaterialTapTargetSize.padded;
    final minHeight = math.max(minimumSize.height + adjustment.dy,
        padded ? kMinInteractiveDimension + adjustment.dy : 0.0);
    // The old popup rows have two TextButton layers around a 20px content area.
    final height = math.max(
            CustomPopupMenuTheme.height + buttonPadding.vertical, minHeight) +
        buttonPadding.vertical;
    return ButtonStyle(
      padding: MaterialStatePropertyAll(
          (buttonPadding * 2).add(padding ?? EdgeInsets.zero)),
      minimumSize: MaterialStatePropertyAll(Size(
          minimumSize.width + adjustment.dx + buttonPadding.horizontal,
          height)),
      fixedSize: MaterialStatePropertyAll(Size(double.infinity, height)),
      visualDensity: VisualDensity.standard,
      tapTargetSize: MaterialTapTargetSize.shrinkWrap,
      textStyle: const MaterialStatePropertyAll(
          TextStyle(fontSize: MenuConfig.fontSize)),
      iconColor: MaterialStateProperty.resolveWith((states) =>
          buttonTheme?.iconColor?.resolve(states) ??
          buttonTheme?.foregroundColor?.resolve(states) ??
          (states.contains(MaterialState.disabled)
              ? theme.colorScheme.onSurface.withOpacity(0.38)
              : theme.colorScheme.primary)),
    );
  }

  // Flutter adds this gap separately from the menu button's padding.
  double get _trailingSpacing =>
      math.max(4, 12 + Theme.of(context).visualDensity.horizontal * 2);

  double get _checkboxWidth {
    final theme = Theme.of(context);
    final checkboxTheme = CheckboxTheme.of(context);
    return ((checkboxTheme.materialTapTargetSize ??
                    theme.materialTapTargetSize) ==
                MaterialTapTargetSize.padded
            ? kMinInteractiveDimension
            : kMinInteractiveDimension - 8) +
        (checkboxTheme.visualDensity ?? theme.visualDensity)
            .baseSizeAdjustment
            .dx;
  }

  Widget _buildItem(MenuEntryBase<T> entry) {
    if (entry is MenuEntryDivider<T>) {
      return const Divider(height: CustomPopupMenuTheme.dividerHeight);
    }
    entry.enabled ??= true.obs;
    if (entry is MenuEntrySubMenu<T>) {
      final padding = (entry.padding ?? EdgeInsets.zero).copyWith(
          right: (entry.padding?.right ?? 0) + 4);
      return Obx(() => _NativeSubmenu(
            style: _buttonStyle(padding).copyWith(
              iconColor: MaterialStatePropertyAll<Color?>(entry.enabled!.value
                  ? Theme.of(context).unselectedWidgetColor
                  : entry.disabledStyle().color),
            ),
            menuChildren: entry.enabled!.value
                ? entry.entries.map(_buildItem).toList()
                : const [],
            child: Text(entry.text,
                style: entry.enabled!.value
                    ? entry.enabledStyle(context)
                    : entry.disabledStyle()),
          ));
    }
    if (entry is MenuEntryButton<T>) {
      return Obx(() => MenuItemButton(
            style: _buttonStyle(entry.padding),
            closeOnActivate: entry.dismissOnClicked,
            onPressed: entry.enabled!.value
                ? () {
                    entry.dismissCallback?.call();
                    entry.proc();
                  }
                : null,
            child: SizedBox(
              height: CustomPopupMenuTheme.height,
              child: Align(
                alignment: AlignmentDirectional.centerStart,
                child: entry.childBuilder(entry.enabled!.value
                    ? entry.enabledStyle(context)
                    : entry.disabledStyle()),
              ),
            ),
          ));
    }
    if (entry is MenuEntrySwitchBase<T>) {
      return Obx(() => MenuItemButton(
            style: _buttonStyle(entry.padding),
            closeOnActivate: entry.dismissOnClicked,
            onPressed: entry.isEnabled
                ? () {
                    entry.dismissCallback?.call();
                    entry.setOption(!entry.curOption.value);
                  }
                : null,
            trailingIcon: ExcludeFocus(
              child: IgnorePointer(
                child: SizedBox(
                  width: _checkboxWidth - _trailingSpacing,
                  height: CustomPopupMenuTheme.height,
                  child: Transform.translate(
                    // Match the 24px favorite/delete icons with 4px right inset.
                    offset:
                        Offset((_checkboxWidth - _trailingSpacing) / 2 - 16, 0),
                    child: Transform.scale(
                      scale: MenuConfig.iconScale,
                      child: Checkbox(
                        value: entry.curOption.value,
                        onChanged: entry.isEnabled ? (_) {} : null,
                      ),
                    ),
                  ),
                ),
              ),
            ),
            child: Text(entry.text,
                style: entry.isEnabled
                    ? entry.enabledStyle(context)
                    : entry.disabledStyle()),
          ));
    }
    debugPrint('Unsupported native popup menu entry: ${entry.runtimeType}');
    return const SizedBox.shrink();
  }
}

class _NativeSubmenu extends StatefulWidget {
  final ButtonStyle style;
  final List<Widget> menuChildren;
  final Widget child;

  const _NativeSubmenu({
    required this.style,
    required this.menuChildren,
    required this.child,
  });

  @override
  State<_NativeSubmenu> createState() => _NativeSubmenuState();
}

class _NativeSubmenuState extends State<_NativeSubmenu> {
  final _controller = MenuController();
  late final _focusNode = FocusNode(onKeyEvent: (_, event) {
    if (event.logicalKey == LogicalKeyboardKey.escape) _cancelHoverOpen();
    return KeyEventResult.ignored;
  });
  Timer? _hoverTimer;
  final _buttonKey = GlobalKey();
  final _itemKey = GlobalKey();
  Offset _offset = Offset.zero;

  @override
  void dispose() {
    _cancelHoverOpen();
    _focusNode.dispose();
    super.dispose();
  }

  void _cancelHoverOpen() {
    _hoverTimer?.cancel();
    _hoverTimer = null;
  }

  void _onHover(bool hovering) {
    _cancelHoverOpen();
    if (hovering) {
      _focusNode.requestFocus();
      if (!_controller.isOpen) {
        _hoverTimer = Timer(const Duration(milliseconds: 200), () {
          if (mounted && widget.menuChildren.isNotEmpty) _controller.open();
        });
      }
    }
  }

  void _toggleMenu() {
    _cancelHoverOpen();
    _focusNode.requestFocus();
    if (_controller.isOpen) {
      _controller.close();
    } else {
      _controller.open();
    }
  }

  void _positionSubmenu() {
    // Submenu dimensions are available after its overlay has been laid out.
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted || !_controller.isOpen) return;
      final buttonContext = _buttonKey.currentContext;
      final button = buttonContext?.findRenderObject();
      final item = _itemKey.currentContext?.findRenderObject();
      if (buttonContext == null || button is! RenderBox || item is! RenderBox) {
        return;
      }
      final overlay = Overlay.of(buttonContext).context.findRenderObject();
      if (overlay is! RenderBox) return;
      final anchor =
          button.localToGlobal(Offset.zero, ancestor: overlay) & button.size;
      final screens = DisplayFeatureSubScreen.subScreensInBounds(
          Offset.zero & overlay.size,
          DisplayFeatureSubScreen.avoidBounds(MediaQuery.of(buttonContext)));
      final screen = screens.reduce((a, b) =>
          (a.center - anchor.center).distanceSquared <
                  (b.center - anchor.center).distanceSquared
              ? a
              : b);
      final leftSpace = anchor.left - screen.left;
      final rightSpace = screen.right - anchor.right;
      var offset = Offset.zero;
      if (item.size.width > leftSpace && item.size.width > rightSpace) {
        if (Directionality.of(buttonContext) == TextDirection.ltr &&
            leftSpace > rightSpace) {
          offset = Offset(screen.left - anchor.right, 0);
        } else if (Directionality.of(buttonContext) == TextDirection.rtl &&
            rightSpace > leftSpace) {
          offset = Offset(anchor.left - screen.right, 0);
        }
      }
      if (_offset != offset) {
        setState(() => _offset = offset);
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    final enabled = widget.menuChildren.isNotEmpty;
    final buttonStyle = widget.style
        .merge(MenuButtonTheme.of(context).style)
        .merge(const MenuItemButton().defaultStyleOf(context));
    final spacing =
        math.max(4.0, 12 + Theme.of(context).visualDensity.horizontal * 2);
    final menuPadding = MenuTheme.of(context)
        .style
        ?.padding
        ?.resolve({})
        ?.resolve(Directionality.of(context));
    return Focus(
      canRequestFocus: false,
      onFocusChange: (focused) {
        if (!focused) {
          _cancelHoverOpen();
          _controller.close();
        }
      },
      child: MenuAnchor(
        key: _buttonKey,
        controller: _controller,
        childFocusNode: _focusNode,
        onOpen: () {
          _cancelHoverOpen();
          setState(() {});
          _positionSubmenu();
        },
        onClose: _cancelHoverOpen,
        alignmentOffset: _offset - Offset(0, menuPadding?.top ?? 0),
        menuChildren: [
          for (var i = 0; i < widget.menuChildren.length; i++)
            if (i == 0)
              KeyedSubtree(key: _itemKey, child: widget.menuChildren[i])
            else
              widget.menuChildren[i],
        ],
        builder: (context, controller, child) => MergeSemantics(
          child: Semantics(
            expanded: enabled && controller.isOpen,
            child: TextButton(
              focusNode: _focusNode,
              onHover: enabled ? _onHover : null,
              onPressed: enabled ? _toggleMenu : null,
              isSemanticButton: null,
              style: buttonStyle,
              child: Row(children: [
                Expanded(child: ClipRect(child: widget.child)),
                SizedBox(width: spacing),
                const Icon(Icons.arrow_right, size: 24),
              ]),
            ),
          ),
        ),
      ),
    );
  }
}
