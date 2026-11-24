import React, { useEffect, useState } from 'react';
import {
  ArrowLeft,
  ChevronRight,
  Info,
  Link2,
  Monitor,
  Network,
  Server,
  Settings,
} from 'lucide-react';
import { probeCodecs } from '../../lib/decoder';
import type { DisplayPrefs, ImageQualityPreset, PreferredCodec } from '../../lib/displayPrefs';
import { t, type Locale, type LocalePreference } from '../../lib/i18n';
import type { ThemePreference } from '../../lib/theme';
import { IdServerModal, type ServerField } from './IdServerModal';

export type SettingsSection = 'general' | 'network' | 'display' | 'about';

interface SettingsViewProps {
  locale: Locale;
  localePreference: LocalePreference;
  theme: ThemePreference;
  displayPrefs: DisplayPrefs;
  section: SettingsSection;
  directIpEnabled: boolean;
  idServer: string;
  relayServer: string;
  serverKey: string;
  forceRelay: boolean;
  gatewayToken: string;
  authRequired: boolean;
  touchMode: boolean;
  diagnosticsOpen: boolean;
  serverDialogOpen: boolean;
  idServerError: boolean;
  onChangeSection: (next: SettingsSection) => void;
  onChangeLocale: (next: LocalePreference) => void;
  onChangeTheme: (next: ThemePreference) => void;
  onUpdateDisplayPrefs: (patch: Partial<DisplayPrefs>) => void;
  onToggleDirectIp: () => void;
  onToggleTouchMode: () => void;
  onToggleDiagnostics: () => void;
  onChangeServerField: (field: ServerField, value: string) => void;
  onChangeForceRelay: (checked: boolean) => void;
  onSetServerDialogOpen: (open: boolean) => void;
  onClearIdServerError: () => void;
  onBack: () => void;
}

const THEME_OPTIONS: ReadonlyArray<{ id: ThemePreference; key: 'themeLight' | 'themeDark' | 'themeSystem' }> = [
  { id: 'light', key: 'themeLight' },
  { id: 'dark', key: 'themeDark' },
  { id: 'system', key: 'themeSystem' },
];

const QUALITY_OPTIONS: ReadonlyArray<{
  id: ImageQualityPreset;
  key: 'qualityGoodImage' | 'qualityBalanced' | 'qualityOptimizeReaction' | 'qualityCustom';
}> = [
  { id: 'best', key: 'qualityGoodImage' },
  { id: 'balanced', key: 'qualityBalanced' },
  { id: 'low', key: 'qualityOptimizeReaction' },
  { id: 'custom', key: 'qualityCustom' },
];

const CODEC_OPTIONS: ReadonlyArray<{ id: PreferredCodec; label?: string }> = [
  { id: 'auto' },
  { id: 'vp8', label: 'VP8' },
  { id: 'vp9', label: 'VP9' },
  { id: 'h264', label: 'H.264' },
  { id: 'h265', label: 'H.265' },
  { id: 'av1', label: 'AV1' },
];

function GeneralSection({
  locale,
  localePreference,
  theme,
  diagnosticsOpen,
  touchMode,
  onChangeTheme,
  onChangeLocale,
  onToggleDiagnostics,
  onToggleTouchMode,
}: Pick<
  SettingsViewProps,
  | 'locale'
  | 'localePreference'
  | 'theme'
  | 'diagnosticsOpen'
  | 'touchMode'
  | 'onChangeTheme'
  | 'onChangeLocale'
  | 'onToggleDiagnostics'
  | 'onToggleTouchMode'
>) {
  return (
    <div className="flex flex-col gap-4">
      <section className="rd-settings-card">
        <h2 className="rd-settings-card-title">{t(locale, 'themeTitle')}</h2>
        <div className="flex flex-col gap-1" role="radiogroup" aria-label={t(locale, 'themeTitle')}>
          {THEME_OPTIONS.map((opt) => (
            <button
              key={opt.id}
              type="button"
              role="radio"
              aria-checked={theme === opt.id}
              data-testid={`settings-theme-${opt.id}`}
              className={`rd-radio-row${theme === opt.id ? ' active' : ''}`}
              onClick={() => onChangeTheme(opt.id)}
            >
              <span className="rd-radio-dot" aria-hidden="true" />
              <span>{t(locale, opt.key)}</span>
            </button>
          ))}
        </div>
      </section>

      <section className="rd-settings-card">
        <h2 className="rd-settings-card-title">{t(locale, 'languageLabel')}</h2>
        <select
          className="rd-select"
          data-testid="settings-language-select"
          aria-label={t(locale, 'languageLabel')}
          value={localePreference}
          onChange={(e) => {
            const v = e.target.value;
            onChangeLocale(v === 'zh' || v === 'en' ? v : 'default');
          }}
        >
          <option value="default">{t(locale, 'languageDefault')}</option>
          <option value="zh">简体中文</option>
          <option value="en">English</option>
        </select>
      </section>

      <section className="rd-settings-card">
        <h2 className="rd-settings-card-title">{t(locale, 'otherSectionTitle')}</h2>
        <div className="flex flex-col gap-1">
          <label className="rd-check-row">
            <input
              type="checkbox"
              data-testid="settings-toggle-diagnostics"
              checked={diagnosticsOpen}
              onChange={onToggleDiagnostics}
            />
            <span>{t(locale, 'diagnosticsOverlayLabel')}</span>
          </label>

          <label className="rd-check-row">
            <input
              type="checkbox"
              data-testid="settings-toggle-touch-mode"
              checked={touchMode}
              onChange={onToggleTouchMode}
            />
            <span>{t(locale, 'touchModeOptionLabel')}</span>
          </label>
        </div>
      </section>
    </div>
  );
}

function NetworkSection({
  locale,
  idServer,
  directIpEnabled,
  onOpenServerDialog,
  onToggleDirectIp,
}: {
  locale: Locale;
  idServer: string;
  directIpEnabled: boolean;
  onOpenServerDialog: () => void;
  onToggleDirectIp: () => void;
}) {
  return (
    <div className="flex flex-col gap-4">
      <section className="rd-settings-card">
        <h2 className="rd-settings-card-title">{t(locale, 'settingsNetwork')}</h2>

        <button
          type="button"
          className="rd-network-action-row"
          data-testid="open-id-server-dialog"
          onClick={onOpenServerDialog}
        >
          <Server size={18} className="text-[var(--accent)] shrink-0" />
          <div className="min-w-0 flex-1 text-left">
            <div className="text-sm font-medium text-[var(--ink)]">
              {t(locale, 'idRelayServerTitle')}
            </div>
            {idServer.trim() && (
              <div className="text-xs text-[var(--ink-soft)] truncate font-mono-tabular mt-0.5">
                {idServer.trim()}
              </div>
            )}
          </div>
          <ChevronRight size={16} className="text-[var(--ink-soft)] shrink-0" />
        </button>

        <div className="my-2.5 h-px bg-[var(--rd-divider)]" />

        <div className="flex items-center justify-between gap-3 py-1 px-2">
          <div className="flex items-center gap-2.5 min-w-0">
            <Network size={17} className="text-[var(--accent)] shrink-0" />
            <span className="text-sm font-medium text-[var(--ink)]">
              {t(locale, 'enableDirectIpLabel')}
            </span>
          </div>
          <button
            type="button"
            role="switch"
            aria-checked={directIpEnabled}
            aria-label={t(locale, 'enableDirectIpLabel')}
            data-testid="settings-toggle-direct-ip"
            className={`rd-switch${directIpEnabled ? ' on' : ''}`}
            onClick={onToggleDirectIp}
          >
            <span className="rd-switch-thumb" />
          </button>
        </div>
      </section>
    </div>
  );
}

function DisplaySection({
  locale,
  displayPrefs,
  diagnosticsOpen,
  onUpdateDisplayPrefs,
  onToggleDiagnostics,
}: {
  locale: Locale;
  displayPrefs: DisplayPrefs;
  diagnosticsOpen: boolean;
  onUpdateDisplayPrefs: (patch: Partial<DisplayPrefs>) => void;
  onToggleDiagnostics: () => void;
}) {
  return (
    <div className="flex flex-col gap-4">
      <section className="rd-settings-card">
        <h2 className="rd-settings-card-title">{t(locale, 'defaultViewStyle')}</h2>
        <div
          className="flex flex-col gap-1"
          role="radiogroup"
          aria-label={t(locale, 'defaultViewStyle')}
        >
          <button
            type="button"
            role="radio"
            aria-checked={displayPrefs.viewStyle === 'original'}
            className={`rd-radio-row${displayPrefs.viewStyle === 'original' ? ' active' : ''}`}
            data-testid="settings-view-original"
            onClick={() => onUpdateDisplayPrefs({ viewStyle: 'original' })}
          >
            <span className="rd-radio-dot" aria-hidden="true" />
            <span>{t(locale, 'viewStyleOriginal')}</span>
          </button>
          <button
            type="button"
            role="radio"
            aria-checked={displayPrefs.viewStyle === 'fit'}
            className={`rd-radio-row${displayPrefs.viewStyle === 'fit' ? ' active' : ''}`}
            data-testid="settings-view-fit"
            onClick={() => onUpdateDisplayPrefs({ viewStyle: 'fit' })}
          >
            <span className="rd-radio-dot" aria-hidden="true" />
            <span>{t(locale, 'viewStyleFit')}</span>
          </button>
        </div>
      </section>

      <section className="rd-settings-card">
        <h2 className="rd-settings-card-title">{t(locale, 'defaultScrollStyle')}</h2>
        <div
          className="flex flex-col gap-1"
          role="radiogroup"
          aria-label={t(locale, 'defaultScrollStyle')}
        >
          <button
            type="button"
            role="radio"
            aria-checked={displayPrefs.scrollStyle === 'auto'}
            className={`rd-radio-row${displayPrefs.scrollStyle === 'auto' ? ' active' : ''}`}
            data-testid="settings-scroll-auto"
            onClick={() => onUpdateDisplayPrefs({ scrollStyle: 'auto' })}
          >
            <span className="rd-radio-dot" aria-hidden="true" />
            <span>{t(locale, 'scrollStyleAuto')}</span>
          </button>
          <button
            type="button"
            role="radio"
            aria-checked={displayPrefs.scrollStyle === 'scrollbar'}
            className={`rd-radio-row${displayPrefs.scrollStyle === 'scrollbar' ? ' active' : ''}`}
            data-testid="settings-scroll-scrollbar"
            onClick={() => onUpdateDisplayPrefs({ scrollStyle: 'scrollbar' })}
          >
            <span className="rd-radio-dot" aria-hidden="true" />
            <span>{t(locale, 'scrollStyleBar')}</span>
          </button>
        </div>
      </section>

      <section className="rd-settings-card">
        <h2 className="rd-settings-card-title">{t(locale, 'defaultImageQuality')}</h2>
        <div
          className="flex flex-col gap-1"
          role="radiogroup"
          aria-label={t(locale, 'defaultImageQuality')}
        >
          {QUALITY_OPTIONS.map((opt) => (
            <button
              key={opt.id}
              type="button"
              role="radio"
              aria-checked={displayPrefs.qualityPreset === opt.id}
              className={`rd-radio-row${displayPrefs.qualityPreset === opt.id ? ' active' : ''}`}
              data-testid={`settings-quality-${opt.id}`}
              onClick={() => onUpdateDisplayPrefs({ qualityPreset: opt.id })}
            >
              <span className="rd-radio-dot" aria-hidden="true" />
              <span>{t(locale, opt.key)}</span>
            </button>
          ))}
        </div>

        {displayPrefs.qualityPreset === 'custom' && (
          <div className="rd-slider-group" data-testid="settings-custom-quality-controls">
            <label className="rd-slider-row">
              <input
                type="range"
                className="rd-range"
                min={10}
                max={200}
                step={5}
                data-testid="settings-custom-bitrate"
                aria-label={t(locale, 'bitrateLabel')}
                value={displayPrefs.customBitrate}
                onChange={(e) => onUpdateDisplayPrefs({ customBitrate: Number(e.target.value) })}
              />
              <span className="rd-slider-val font-mono-tabular">{displayPrefs.customBitrate}%</span>
              <span className="rd-slider-label">{t(locale, 'bitrateLabel')}</span>
            </label>

            <label className="rd-slider-row">
              <input
                type="range"
                className="rd-range"
                min={10}
                max={60}
                step={5}
                data-testid="settings-custom-fps"
                aria-label={t(locale, 'fpsLabel')}
                value={displayPrefs.customFps}
                onChange={(e) => onUpdateDisplayPrefs({ customFps: Number(e.target.value) })}
              />
              <span className="rd-slider-val font-mono-tabular">{displayPrefs.customFps}</span>
              <span className="rd-slider-label">{t(locale, 'fpsLabel')}</span>
            </label>
          </div>
        )}
      </section>

      <section className="rd-settings-card">
        <h2 className="rd-settings-card-title">{t(locale, 'defaultCodec')}</h2>
        <div
          className="flex flex-col gap-1"
          role="radiogroup"
          aria-label={t(locale, 'defaultCodec')}
        >
          {CODEC_OPTIONS.map((opt) => (
            <button
              key={opt.id}
              type="button"
              role="radio"
              aria-checked={displayPrefs.codec === opt.id}
              className={`rd-radio-row${displayPrefs.codec === opt.id ? ' active' : ''}`}
              data-testid={`settings-codec-${opt.id}`}
              onClick={() => onUpdateDisplayPrefs({ codec: opt.id })}
            >
              <span className="rd-radio-dot" aria-hidden="true" />
              <span>{opt.label ?? t(locale, 'codecAuto')}</span>
            </button>
          ))}
        </div>
      </section>

      <section className="rd-settings-card">
        <h2 className="rd-settings-card-title">{t(locale, 'otherDefaultOptions')}</h2>
        <div className="flex flex-col gap-0.5">
          <label className="rd-check-row">
            <input
              type="checkbox"
              data-testid="settings-opt-view-only"
              checked={displayPrefs.viewOnly}
              onChange={(e) => onUpdateDisplayPrefs({ viewOnly: e.target.checked })}
            />
            <span>{t(locale, 'optViewOnly')}</span>
          </label>

          <label className="rd-check-row">
            <input
              type="checkbox"
              data-testid="settings-opt-remote-cursor"
              checked={displayPrefs.showRemoteCursor}
              onChange={(e) => onUpdateDisplayPrefs({ showRemoteCursor: e.target.checked })}
            />
            <span>{t(locale, 'optShowRemoteCursor')}</span>
          </label>

          <label className="rd-check-row">
            <input
              type="checkbox"
              data-testid="settings-opt-quality-monitor"
              checked={diagnosticsOpen}
              onChange={onToggleDiagnostics}
            />
            <span>{t(locale, 'optShowQualityMonitor')}</span>
          </label>

          <label className="rd-check-row">
            <input
              type="checkbox"
              data-testid="settings-opt-disable-clipboard"
              checked={displayPrefs.disableClipboard}
              onChange={(e) => onUpdateDisplayPrefs({ disableClipboard: e.target.checked })}
            />
            <span>{t(locale, 'optDisableClipboard')}</span>
          </label>

          <label className="rd-check-row">
            <input
              type="checkbox"
              data-testid="settings-opt-lock-after-end"
              checked={displayPrefs.lockAfterSessionEnd}
              onChange={(e) => onUpdateDisplayPrefs({ lockAfterSessionEnd: e.target.checked })}
            />
            <span>{t(locale, 'optLockAfterSessionEnd')}</span>
          </label>

          <label className="rd-check-row">
            <input
              type="checkbox"
              data-testid="settings-opt-reverse-wheel"
              checked={displayPrefs.reverseMouseWheel}
              onChange={(e) => onUpdateDisplayPrefs({ reverseMouseWheel: e.target.checked })}
            />
            <span>{t(locale, 'optReverseMouseWheel')}</span>
          </label>

          <label className="rd-check-row">
            <input
              type="checkbox"
              data-testid="settings-opt-swap-buttons"
              checked={displayPrefs.swapMouseButtons}
              onChange={(e) => onUpdateDisplayPrefs({ swapMouseButtons: e.target.checked })}
            />
            <span>{t(locale, 'optSwapMouseButtons')}</span>
          </label>
        </div>
      </section>
    </div>
  );
}

function AboutSection({
  locale,
  detectedCodecs,
}: {
  locale: Locale;
  detectedCodecs: string[] | null;
}) {
  return (
    <div className="flex flex-col gap-4">
      <section className="rd-settings-card" data-testid="about-rustdesk-card">
        <h2 className="rd-settings-card-title">{t(locale, 'aboutRustDeskTitle')}</h2>
        <div className="rd-about-meta">
          <div>
            <span className="text-[var(--ink-soft)]">{t(locale, 'aboutVersion')}: </span>
            <span className="font-mono-tabular">1.0.0</span>
          </div>
          <div>
            <span className="text-[var(--ink-soft)]">{t(locale, 'aboutProtocol')}: </span>
            <span>RustDesk Native + WebCodecs</span>
          </div>
          <div>
            <span className="text-[var(--ink-soft)]">{t(locale, 'aboutDecoders')}: </span>
            <span className="font-mono-tabular" data-testid="about-detected-codecs">
              {detectedCodecs === null
                ? t(locale, 'aboutDecodersDetecting')
                : detectedCodecs.length > 0
                  ? detectedCodecs.join(' · ')
                  : t(locale, 'aboutDecodersNone')}
            </span>
          </div>
          <a
            href="https://rustdesk.com"
            target="_blank"
            rel="noopener noreferrer"
            className="rd-about-link"
          >
            {t(locale, 'aboutWebsite')}
          </a>
          <a
            href="https://github.com/rustdesk/rustdesk"
            target="_blank"
            rel="noopener noreferrer"
            className="rd-about-link"
          >
            {t(locale, 'aboutGithub')}
          </a>
        </div>

        <div className="rd-about-banner">
          <div className="text-sm font-medium">{t(locale, 'aboutBannerLine1')}</div>
          <div className="text-sm font-semibold">{t(locale, 'aboutBannerLine2')}</div>
        </div>
      </section>
    </div>
  );
}

export function SettingsView(props: SettingsViewProps) {
  const { locale, section, serverDialogOpen, onChangeSection, onSetServerDialogOpen, onBack } =
    props;
  const [detectedCodecs, setDetectedCodecs] = useState<string[] | null>(null);

  useEffect(() => {
    let active = true;
    probeCodecs()
      .then((res) => {
        if (!active) return;
        const list: string[] = [];
        if (res.vp8) list.push('VP8');
        if (res.vp9) list.push('VP9');
        if (res.h264) list.push('H.264');
        if (res.h265) list.push('H.265');
        if (res.av1) list.push('AV1');
        setDetectedCodecs(list);
      })
      .catch(() => {
        if (active) setDetectedCodecs([]);
      });
    return () => {
      active = false;
    };
  }, []);

  const navItems: ReadonlyArray<{
    id: SettingsSection;
    label: string;
    icon: React.ReactNode;
  }> = [
    { id: 'general', label: t(locale, 'settingsGeneral'), icon: <Settings size={17} className="shrink-0" /> },
    { id: 'network', label: t(locale, 'settingsNetwork'), icon: <Link2 size={17} className="shrink-0" /> },
    { id: 'display', label: t(locale, 'settingsDisplay'), icon: <Monitor size={17} className="shrink-0" /> },
    { id: 'about', label: t(locale, 'settingsAbout'), icon: <Info size={17} className="shrink-0" /> },
  ];

  return (
    <div
      className="rd-mobile-shell rd-settings-desktop"
      data-testid="connection-panel"
      onPointerDown={(e) => e.stopPropagation()}
    >
      <aside className="rd-settings-sidebar" data-testid="settings-sidebar">
        <div className="rd-settings-sidebar-header">
          <button
            type="button"
            className="rd-mobile-icon-btn dark rd-settings-back-btn"
            data-testid="mobile-settings-back"
            aria-label={t(locale, 'back')}
            data-tooltip={t(locale, 'back')}
            data-tooltip-pos="bottom-start"
            onClick={onBack}
          >
            <ArrowLeft size={20} />
          </button>
          <h1 className="rd-settings-sidebar-title">{t(locale, 'tabSettings')}</h1>
        </div>

        <nav className="flex flex-col py-2" aria-label={t(locale, 'tabSettings')}>
          {navItems.map((item) => (
            <button
              key={item.id}
              type="button"
              data-testid={`settings-tab-${item.id}`}
              aria-current={section === item.id ? 'page' : undefined}
              className={`rd-sidebar-item${section === item.id ? ' active' : ''}`}
              onClick={() => onChangeSection(item.id)}
            >
              {item.icon}
              <span className="truncate">{item.label}</span>
            </button>
          ))}
        </nav>
      </aside>

      <main className="rd-settings-main">
        <div className="rd-settings-content-inner">
          {section === 'general' && (
            <GeneralSection
              locale={props.locale}
              localePreference={props.localePreference}
              theme={props.theme}
              diagnosticsOpen={props.diagnosticsOpen}
              touchMode={props.touchMode}
              onChangeTheme={props.onChangeTheme}
              onChangeLocale={props.onChangeLocale}
              onToggleDiagnostics={props.onToggleDiagnostics}
              onToggleTouchMode={props.onToggleTouchMode}
            />
          )}

          {section === 'network' && (
            <NetworkSection
              locale={props.locale}
              idServer={props.idServer}
              directIpEnabled={props.directIpEnabled}
              onOpenServerDialog={() => onSetServerDialogOpen(true)}
              onToggleDirectIp={props.onToggleDirectIp}
            />
          )}

          {section === 'display' && (
            <DisplaySection
              locale={props.locale}
              displayPrefs={props.displayPrefs}
              diagnosticsOpen={props.diagnosticsOpen}
              onUpdateDisplayPrefs={props.onUpdateDisplayPrefs}
              onToggleDiagnostics={props.onToggleDiagnostics}
            />
          )}

          {section === 'about' && (
            <AboutSection locale={props.locale} detectedCodecs={detectedCodecs} />
          )}
        </div>
      </main>

      {serverDialogOpen && (
        <IdServerModal
          locale={props.locale}
          idServer={props.idServer}
          relayServer={props.relayServer}
          serverKey={props.serverKey}
          forceRelay={props.forceRelay}
          gatewayToken={props.gatewayToken}
          authRequired={props.authRequired}
          idServerError={props.idServerError}
          onChangeField={props.onChangeServerField}
          onChangeForceRelay={props.onChangeForceRelay}
          onClearError={props.onClearIdServerError}
          onClose={() => onSetServerDialogOpen(false)}
        />
      )}
    </div>
  );
}
