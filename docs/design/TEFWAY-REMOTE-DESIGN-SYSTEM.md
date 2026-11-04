# Tefway Remote — Design System

Status: implemented in `flutter/lib/tefway/`, gate `TEFWAY-REMOTE-UX-02`.
Product copy is Brazilian Portuguese.

## 1. Direction

Enterprise, premium, technical. Clean surfaces, discreetly rounded corners,
one-pixel borders, moderate elevation, strong typographic hierarchy, one primary
action per screen, and status that never relies on colour alone.

Rejected on purpose: neon, heavy gradients, decorative illustrations, marketing
banners, and the pink/orange and cyan/blue gradient cards inherited from
RustDesk.

## 2. Tokens

Single source: `flutter/lib/tefway/design.dart`.

### Colour

| Token | Value | Use |
| --- | --- | --- |
| `brandBlue` | `#164193` | Brand mark background, deep brand surfaces |
| `actionBlue` | `#2878E5` | Primary action, focus ring, busy state |
| `deepNavy` | `#101C30` | Application background |
| `surface` | `#1B2B43` | Cards, dialogs |
| `surfaceElevated` | `#233B56` | Hover/focus surface |
| `inputFill` | `#16253A` | Inputs and inset blocks |
| `textPrimary` | `#F5F8FD` | Primary text |
| `textSecondary` | `#A9BCD7` | Labels, captions, secondary text |
| `border` | `#37516F` | Card and input borders |
| `positive` | `#59C99D` | Ready, available |
| `attention` | `#FFA700` | Active session, advisory notice, pending consent |
| `critical` | `#E76767` | Failure, refusal, destructive action |

Contrast: `textPrimary` on `deepNavy`/`surface` is above WCAG AA for normal text;
`textSecondary` on `surface` sits just above AA at 13 px and is used for labels
and captions, never as the sole carrier of meaning. Status always pairs the
colour with an icon and a text label.

Brand blue is not painted over every button: the primary action uses `actionBlue`
for clarity and the brand stays in the mark and surface accents.

### Spacing, radius, type

- Spacing scale: 4 / 8 / 12 / 16 / 24 / 32 (`TefwaySpacing`).
- Radii: 8 (inputs, chips), 12 (inner blocks), 16 (cards) — never pills.
- Type scale (`TefwayTextStyles`): `appTitle` 18/700, `sectionTitle` 15/600,
  `label` 12/600, `body` 13, `bodySecondary` 13/secondary, `caption` 11,
  `code` 26/600 tabular, `statusLabel` 12/600.
- Identification codes use `FontFeature.tabularFigures()` so changing digits do
  not shift the string.
- Font family: platform default. Inter is not bundled; adding a font file is a
  branding decision for UX-02F, and no network font loading is allowed because
  the product must run offline and portable.

### Theme

`TefwayTheme.data()` derives from the stock dark theme, preserving
`ColorThemeExtension`/`TabbarTheme` extensions so settings and dialogs that read
`MyTheme.color(context)` keep working, and re-points the brand colours, inputs,
elevated/outlined buttons and dialog surfaces. Applied only when
`isTefwayBuild()`; stock RustDesk keeps `MyTheme` untouched.

The Tefway build is always dark: the palette is the product identity, and a
light theme would require a second validated palette. This is a deliberate
scope decision, recorded in the gate report.

## 3. Components

All in `flutter/lib/tefway/widgets.dart` unless noted. Only components used by
more than one surface, or carrying a security meaning, were created.

| Component | Purpose |
| --- | --- |
| `TefwayAppShell` | Page frame: navy background, max width, optional scrolling |
| `TefwaySurfaceCard` | Card surface with border and radius |
| `TefwayBrandMark` | Brand blue square with the support headset glyph |
| `TefwayBrandHeader` | Mark + product name + role subtitle + trailing status |
| `TefwayPrimaryButton` | Filled action, label mandatory |
| `TefwaySecondaryButton` | Outlined action, always visually weaker |
| `TefwayStatusIndicator` | Icon + label + optional detail; semantics label included |
| `TefwayStatusChip` | Compact status for headers and section titles |
| `TefwaySectionTitle` | Section heading with optional trailing widget |
| `TefwayDeviceIdCard` | Label + tabular code + copy action |
| `TefwaySessionCard` | One Master session: destination, time, state, open/end |
| `TefwayEmptyState` | Honest empty state with explanation |
| `TefwayNotice` | Contextual warning with inline "Saiba mais" details |
| `TefwaySupportFooter` | Signature, version, licence attribution |
| `TefwayConsentCard` (`consent.dart`) | Incoming session request: requester, capability switches, Recusar / Permitir acesso |
| `TefwayCapability` (`consent.dart`) | One permission row of the consent card |
| `TefwayCoreErrorNotice` (`notices.dart`) | Real core error surfaced with a retry hook |

The consent card is presentation only: it receives callbacks and never calls
`sendLoginResponse` itself. Those calls stay in
`flutter/lib/desktop/pages/server_page.dart`, beside the click-tampering guard
(`checkClickTime`) the core window already relied on, so the Tefway surface
cannot grant more than the stock window could.

## 4. Accessibility rules applied

- Every status carries icon + text; colour is redundant.
- Buttons keep a text label; icon-only controls are limited to the existing
  toolbar surfaces.
- Focusable controls are stock Material buttons, so Tab traversal and the focus
  highlight come from the framework.
- The identification code is `SelectableText`, so it can be read with a screen
  reader and copied manually.
- Text uses relative line heights and no fixed-height rows around text, so
  scaling does not clip labels.
- Consent capabilities are `Switch.adaptive` with visible text labels, not
  colour-coded tiles.
- No animation was introduced, so there is nothing to reduce for
  `MediaQuery.disableAnimations`.
