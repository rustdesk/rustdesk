import React, { useEffect, useMemo, useRef, useState } from 'react';
import {
  ArrowRight,
  Clock,
  HelpCircle,
  LayoutGrid,
  List,
  Loader2,
  Maximize2,
  Minimize2,
  MoreVertical,
  Search,
  Star,
  X,
} from 'lucide-react';
import { deviceKeyFor, formatPeerId, type SavedDevice } from '../lib/devices';
import type { DisplayPrefs } from '../lib/displayPrefs';
import { statusLabel, t, type Locale, type LocalePreference } from '../lib/i18n';
import { getSavedPassword, type SavedPasswordMap } from '../lib/passwords';
import type { ThemePreference } from '../lib/theme';

export interface ConnectRequest {
  mode?: ConnectionMode;
  peerId?: string;
  idServer?: string;
  relayServer?: string;
  serverKey?: string;
  forceRelay?: boolean;
  host?: string;
  port?: string;
  password?: string;
  remember?: boolean;
}
import { ErrorBanner } from './ErrorBanner';
import { PasswordModal } from './PasswordModal';
import { PeerList } from './PeerList';
import { SettingsView, type SettingsSection } from './settings/SettingsView';

export type ConnectionMode = 'id' | 'direct';
export type ConnectionPhase = 'idle' | 'connecting' | 'live' | 'error';

type ListTab = 'recent' | 'favorites';

export type ConnectionField =
  | 'host'
  | 'port'
  | 'password'
  | 'peerId'
  | 'idServer'
  | 'relayServer'
  | 'serverKey'
  | 'gatewayToken';

interface RustDeskShellProps {
  locale: Locale;
  localePreference: LocalePreference;
  isDesktop: boolean;
  isFullscreen: boolean;
  theme: ThemePreference;
  displayPrefs: DisplayPrefs;
  onChangeLocale: (next: LocalePreference) => void;
  onChangeTheme: (next: ThemePreference) => void;
  onUpdateDisplayPrefs: (patch: Partial<DisplayPrefs>) => void;
  mode: ConnectionMode;
  directIpEnabled: boolean;
  phase: ConnectionPhase;
  host: string;
  port: string;
  password: string;
  peerId: string;
  idServer: string;
  relayServer: string;
  serverKey: string;
  forceRelay: boolean;
  gatewayToken: string;
  authRequired: boolean;
  detail: string;
  decoderError: string;
  touchMode: boolean;
  diagnosticsOpen: boolean;
  devices: SavedDevice[];
  savedPasswords: SavedPasswordMap;
  rememberPassword: boolean;
  onChangeMode: (mode: ConnectionMode) => void;
  onToggleDirectIp: () => void;
  onChangeField: (field: ConnectionField, value: string) => void;
  onChangeForceRelay: (checked: boolean) => void;
  onChangeRememberPassword: (checked: boolean) => void;
  onClearSavedPassword: (deviceKey: string) => void;
  onSelectDevice: (device: SavedDevice) => void;
  onToggleFavorite: (key: string) => void;
  onRemoveDevice: (key: string) => void;
  onToggleTouchMode: () => void;
  onToggleDiagnostics: () => void;
  onToggleFullscreen: () => void;
  onConnect: (req?: ConnectRequest) => void;
  onDisconnect: () => void;
  onDismissError: () => void;
  onClose: () => void;
}

export function RustDeskShell({
  locale,
  localePreference,
  isDesktop,
  isFullscreen,
  theme,
  displayPrefs,
  onChangeLocale,
  onChangeTheme,
  onUpdateDisplayPrefs,
  mode,
  directIpEnabled,
  phase,
  host,
  port,
  password,
  peerId,
  idServer,
  relayServer,
  serverKey,
  forceRelay,
  gatewayToken,
  authRequired,
  detail,
  decoderError,
  touchMode,
  diagnosticsOpen,
  devices,
  savedPasswords,
  rememberPassword,
  onChangeMode,
  onToggleDirectIp,
  onChangeField,
  onChangeForceRelay,
  onChangeRememberPassword,
  onClearSavedPassword,
  onSelectDevice,
  onToggleFavorite,
  onRemoveDevice,
  onToggleTouchMode,
  onToggleDiagnostics,
  onToggleFullscreen,
  onConnect,
  onDisconnect,
  onDismissError,
  onClose,
}: RustDeskShellProps) {
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [settingsSection, setSettingsSection] = useState<SettingsSection>('general');
  const [serverDialogOpen, setServerDialogOpen] = useState(false);
  const [idServerError, setIdServerError] = useState(false);

  const [listTab, setListTab] = useState<ListTab>('recent');
  const [viewLayout, setViewLayout] = useState<'grid' | 'list'>(isDesktop ? 'grid' : 'list');
  const [searchOpen, setSearchOpen] = useState(false);
  const [searchQuery, setSearchQuery] = useState('');
  const [helpHover, setHelpHover] = useState(false);
  const [helpPinned, setHelpPinned] = useState(false);
  const helpVisible = helpHover || helpPinned;

  const [heroInputError, setHeroInputError] = useState(false);
  const [heroErrorShake, setHeroErrorShake] = useState(false);
  const [passwordPromptOpen, setPasswordPromptOpen] = useState(false);
  const [pendingMode, setPendingMode] = useState<ConnectionMode>(mode);

  const peerInputRef = useRef<HTMLInputElement>(null);
  const hostInputRef = useRef<HTMLInputElement>(null);
  const helpAnchorRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!helpVisible) return;
    const handlePointerDown = (ev: PointerEvent) => {
      if (helpAnchorRef.current && !helpAnchorRef.current.contains(ev.target as Node)) {
        setHelpPinned(false);
        setHelpHover(false);
      }
    };
    const handleKeyDown = (ev: KeyboardEvent) => {
      if (ev.key === 'Escape') {
        setHelpPinned(false);
        setHelpHover(false);
      }
    };
    document.addEventListener('pointerdown', handlePointerDown);
    window.addEventListener('keydown', handleKeyDown);
    return () => {
      document.removeEventListener('pointerdown', handlePointerDown);
      window.removeEventListener('keydown', handleKeyDown);
    };
  }, [helpVisible]);

  const isLive = phase === 'live';
  const isConnecting = phase === 'connecting';
  const activeMode: ConnectionMode = directIpEnabled ? mode : 'id';
  const effectiveLayout = isDesktop ? viewLayout : 'list';

  const formattedIdValue = useMemo(() => {
    const clean = peerId.replace(/\s+/g, '');
    if (/^\d+$/.test(clean) && clean.length > 3) {
      return clean.replace(/(\d{3})(?=\d)/g, '$1 ');
    }
    return peerId;
  }, [peerId]);

  const filteredDevices = useMemo(() => {
    const base = listTab === 'favorites' ? devices.filter((d) => d.favorite) : devices;
    const q = searchQuery.trim().toLowerCase();
    if (!q) return base;
    const compactQ = q.replace(/\s+/g, '');
    return base.filter((d) => {
      return (
        (d.peerId ?? '').toLowerCase().includes(compactQ) ||
        (d.host ?? '').toLowerCase().includes(q) ||
        (d.hostname ?? '').toLowerCase().includes(q) ||
        (d.platform ?? '').toLowerCase().includes(q)
      );
    });
  }, [devices, listTab, searchQuery]);

  const activeTargetLabel =
    pendingMode === 'id'
      ? formatPeerId(peerId)
      : `${host.trim()}${port.trim() && port.trim() !== '21118' ? `:${port.trim()}` : ''}`;

  const triggerHeroInputError = () => {
    setHeroInputError(true);
    setHeroErrorShake(false);
    window.requestAnimationFrame(() => setHeroErrorShake(true));
  };

  const openNetworkServerSetup = (highlightIdServer = false) => {
    setSettingsSection('network');
    setSettingsOpen(true);
    setIdServerError(highlightIdServer);
    setServerDialogOpen(true);
  };

  const pendingDeviceKey = deviceKeyFor({
    mode: pendingMode,
    peerId,
    host,
    port,
  });

  const handleHeroSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (activeMode === 'id') {
      if (!peerId.replace(/\s+/g, '')) {
        triggerHeroInputError();
        peerInputRef.current?.focus();
        return;
      }
      if (!idServer.trim()) {
        openNetworkServerSetup(true);
        return;
      }
    } else if (!host.trim()) {
      triggerHeroInputError();
      hostInputRef.current?.focus();
      return;
    }

    setHeroInputError(false);
    setHeroErrorShake(false);
    setPendingMode(activeMode);

    const key = deviceKeyFor({ mode: activeMode, peerId, host, port });
    const saved = getSavedPassword(savedPasswords, key);
    const needsTokenPrompt = authRequired && !gatewayToken.trim();
    if (saved && !needsTokenPrompt && phase !== 'error') {
      onChangeField('password', saved);
      onConnect({ mode: activeMode, password: saved, remember: true });
      return;
    }

    if (saved) {
      onChangeField('password', saved);
    }
    setPasswordPromptOpen(true);
  };

  const handleDeviceRowTap = (device: SavedDevice) => {
    onSelectDevice(device);
    setHeroInputError(false);
    if (device.mode === 'id' && !(device.idServer?.trim() || idServer.trim())) {
      openNetworkServerSetup(true);
      return;
    }
    setPendingMode(device.mode);

    const key = deviceKeyFor(device);
    const saved = getSavedPassword(savedPasswords, key);
    const needsTokenPrompt = authRequired && !gatewayToken.trim();
    if (saved && !needsTokenPrompt) {
      onChangeField('password', saved);
      onConnect({
        mode: device.mode,
        peerId: device.peerId,
        idServer: device.idServer || idServer,
        relayServer: device.relayServer ?? relayServer,
        serverKey: device.serverKey ?? serverKey,
        forceRelay: device.forceRelay ?? forceRelay,
        host: device.host,
        port: device.port || '21118',
        password: saved,
        remember: true,
      });
      return;
    }

    onChangeField('password', saved);
    setPasswordPromptOpen(true);
  };

  const handleConfirmPasswordConnect = (e: React.FormEvent) => {
    e.preventDefault();
    setPasswordPromptOpen(false);
    onConnect({ mode: pendingMode, password, remember: rememberPassword });
  };

  const handleIdInputChange = (raw: string) => {
    setHeroInputError(false);
    const digitsOnly = raw.replace(/\s+/g, '');
    onChangeField('peerId', /^\d*$/.test(digitsOnly) ? digitsOnly : raw);
  };

  if (settingsOpen) {
    return (
      <SettingsView
        locale={locale}
        localePreference={localePreference}
        theme={theme}
        displayPrefs={displayPrefs}
        section={settingsSection}
        directIpEnabled={directIpEnabled}
        idServer={idServer}
        relayServer={relayServer}
        serverKey={serverKey}
        forceRelay={forceRelay}
        gatewayToken={gatewayToken}
        authRequired={authRequired}
        touchMode={touchMode}
        diagnosticsOpen={diagnosticsOpen}
        serverDialogOpen={serverDialogOpen}
        idServerError={idServerError}
        onChangeSection={setSettingsSection}
        onChangeLocale={onChangeLocale}
        onChangeTheme={onChangeTheme}
        onUpdateDisplayPrefs={onUpdateDisplayPrefs}
        onToggleDirectIp={onToggleDirectIp}
        onToggleTouchMode={onToggleTouchMode}
        onToggleDiagnostics={onToggleDiagnostics}
        onChangeServerField={onChangeField}
        onChangeForceRelay={onChangeForceRelay}
        onSetServerDialogOpen={(open) => {
          setServerDialogOpen(open);
          if (!open) setIdServerError(false);
        }}
        onClearIdServerError={() => setIdServerError(false)}
        onBack={() => {
          setServerDialogOpen(false);
          setIdServerError(false);
          setSettingsOpen(false);
        }}
      />
    );
  }

  const fullscreenLabel = isFullscreen ? t(locale, 'exitFullscreen') : t(locale, 'enterFullscreen');

  return (
    <div
      className={`rd-mobile-shell${isFullscreen ? ' is-fullscreen' : ''}`}
      data-testid="connection-panel"
      data-fullscreen={isFullscreen ? 'true' : 'false'}
      onPointerDown={(e) => e.stopPropagation()}
    >
      {isFullscreen ? (
        <div className="rd-fs-floating-actions" data-testid="fullscreen-floating-actions">
          {isLive && (
            <button
              type="button"
              className="rd-mobile-icon-btn dark"
              style={{ width: 36, height: 36 }}
              aria-label={t(locale, 'close')}
              data-tooltip={t(locale, 'close')}
              data-tooltip-pos="bottom-end"
              onClick={onClose}
            >
              <X size={18} />
            </button>
          )}
          <button
            type="button"
            className="rd-mobile-icon-btn dark"
            style={{ width: 36, height: 36 }}
            data-testid="toggle-fullscreen"
            aria-label={fullscreenLabel}
            aria-pressed={isFullscreen}
            data-tooltip={fullscreenLabel}
            data-tooltip-pos="bottom-end"
            onClick={onToggleFullscreen}
          >
            <Minimize2 size={17} />
          </button>
          <button
            type="button"
            className="rd-mobile-icon-btn dark"
            style={{ width: 36, height: 36 }}
            data-testid="mobile-open-settings"
            aria-label={t(locale, 'openSettings')}
            data-tooltip={t(locale, 'tabSettings')}
            data-tooltip-pos="bottom-end"
            onClick={() => setSettingsOpen(true)}
          >
            <MoreVertical size={18} />
          </button>
        </div>
      ) : (
        <header className="rd-mobile-topbar" data-testid="main-topbar">
          <div className="w-20 flex items-center justify-start">
            {isLive && (
              <button
                type="button"
                className="rd-mobile-icon-btn"
                aria-label={t(locale, 'close')}
                data-tooltip={t(locale, 'close')}
                data-tooltip-pos="bottom-start"
                onClick={onClose}
              >
                <X size={20} />
              </button>
            )}
          </div>

          <h1 className="text-[18px] font-semibold text-[var(--rd-topbar-ink)] m-0 tracking-[0.01em] truncate">
            RustDesk
          </h1>

          <div className="w-20 flex items-center justify-end gap-0.5">
            <button
              type="button"
              className="rd-mobile-icon-btn"
              data-testid="toggle-fullscreen"
              aria-label={fullscreenLabel}
              aria-pressed={isFullscreen}
              data-tooltip={fullscreenLabel}
              data-tooltip-pos="bottom-end"
              onClick={onToggleFullscreen}
            >
              <Maximize2 size={18} />
            </button>
            <button
              type="button"
              className="rd-mobile-icon-btn"
              data-testid="mobile-open-settings"
              aria-label={t(locale, 'openSettings')}
              data-tooltip={t(locale, 'tabSettings')}
              data-tooltip-pos="bottom-end"
              onClick={() => setSettingsOpen(true)}
            >
              <MoreVertical size={20} />
            </button>
          </div>
        </header>
      )}

      <main className="rd-mobile-scroll rd-mobile-home-body">
        {/* Hero Remote ID / Direct IP Input Block */}
        <section className="rd-hero-container">
          <div className="flex items-center justify-between gap-2 mb-2.5">
            <div className="flex items-center gap-1.5 min-w-0">
              <h2 className="text-[18px] font-medium text-[var(--ink)] m-0">
                {t(locale, 'remoteDesktop')}
              </h2>

              <div
                ref={helpAnchorRef}
                className="relative inline-flex items-center"
                onMouseEnter={() => {
                  if (window.matchMedia?.('(hover: hover)').matches) {
                    setHelpHover(true);
                  }
                }}
                onMouseLeave={() => {
                  if (window.matchMedia?.('(hover: hover)').matches) {
                    setHelpHover(false);
                  }
                }}
              >
                <button
                  type="button"
                  className="inline-flex items-center justify-center w-8 h-8 rounded-full text-[var(--ink-soft)] hover:bg-[var(--rd-hover-bg)] border-0 bg-transparent"
                  data-testid="remote-help-btn"
                  aria-label={t(locale, 'helpButtonAria')}
                  aria-expanded={helpVisible}
                  aria-controls="remote-help-popover"
                  onClick={() => {
                    if (helpPinned) {
                      setHelpPinned(false);
                      setHelpHover(false);
                    } else {
                      setHelpPinned(true);
                    }
                  }}
                >
                  <HelpCircle size={17} />
                </button>

                {helpVisible && (
                  <div
                    id="remote-help-popover"
                    className="rd-help-popover"
                    data-testid="remote-help-popover"
                    role="tooltip"
                  >
                    {t(locale, 'helpRemoteDesktop')}
                  </div>
                )}
              </div>
            </div>

            {directIpEnabled && (
              <div
                className="rd-mode-switcher"
                role="tablist"
                aria-label={t(locale, 'connectionModeAria')}
              >
                <button
                  type="button"
                  role="tab"
                  aria-selected={activeMode === 'id'}
                  data-testid="mode-id"
                  className={`rd-mode-switcher-btn${activeMode === 'id' ? ' active' : ''}`}
                  onClick={() => {
                    setHeroInputError(false);
                    onChangeMode('id');
                  }}
                >
                  {t(locale, 'modeId')}
                </button>
                <button
                  type="button"
                  role="tab"
                  aria-selected={activeMode === 'direct'}
                  data-testid="mode-direct"
                  className={`rd-mode-switcher-btn${activeMode === 'direct' ? ' active' : ''}`}
                  onClick={() => {
                    setHeroInputError(false);
                    onChangeMode('direct');
                  }}
                >
                  {t(locale, 'modeDirect')}
                </button>
              </div>
            )}
          </div>

          <form onSubmit={handleHeroSubmit} noValidate>
            {activeMode === 'id' ? (
              <div
                className={`rd-hero-box${heroInputError ? ' has-error' : ''}${
                  heroErrorShake ? ' rd-shake' : ''
                }`}
                data-testid="hero-input-box"
                onAnimationEnd={() => setHeroErrorShake(false)}
              >
                <div className="flex-1 min-w-0 flex flex-col justify-center">
                  <label
                    htmlFor="rustdesk-peer-id"
                    className={`transition-all leading-none ${
                      peerId ? 'text-[11px] font-medium text-[var(--ink-soft)] mb-1' : 'sr-only'
                    }`}
                  >
                    {t(locale, 'remoteIdLabel')}
                  </label>
                  <input
                    ref={peerInputRef}
                    id="rustdesk-peer-id"
                    type="text"
                    inputMode="numeric"
                    data-testid="peer-id"
                    aria-invalid={heroInputError || undefined}
                    aria-describedby={heroInputError ? 'rustdesk-hero-error' : undefined}
                    className={`rd-hero-input font-mono-tabular${peerId ? ' has-value' : ''}`}
                    value={formattedIdValue}
                    autoComplete="off"
                    spellCheck={false}
                    placeholder={t(locale, 'remoteIdLabel')}
                    onChange={(e) => handleIdInputChange(e.target.value)}
                  />
                </div>

                {peerId && (
                  <button
                    type="button"
                    className="rd-hero-action-btn"
                    aria-label={t(locale, 'clearInput')}
                    data-tooltip={t(locale, 'clearInput')}
                    data-tooltip-pos="top"
                    onClick={() => {
                      setHeroInputError(false);
                      onChangeField('peerId', '');
                    }}
                  >
                    <X size={20} />
                  </button>
                )}

                <button
                  type="submit"
                  className="rd-hero-action-btn primary-arrow"
                  data-testid="connect"
                  aria-label={t(locale, 'connect')}
                  data-tooltip={t(locale, 'connect')}
                  data-tooltip-pos="top-end"
                  disabled={isConnecting}
                >
                  <ArrowRight size={24} />
                </button>
              </div>
            ) : (
              <div
                className={`rd-hero-box direct${heroInputError ? ' has-error' : ''}${
                  heroErrorShake ? ' rd-shake' : ''
                }`}
                data-testid="hero-input-box"
                onAnimationEnd={() => setHeroErrorShake(false)}
              >
                <div className="flex-1 min-w-0 flex items-center gap-2">
                  <input
                    ref={hostInputRef}
                    type="text"
                    data-testid="host"
                    aria-invalid={heroInputError || undefined}
                    aria-describedby={heroInputError ? 'rustdesk-hero-error' : undefined}
                    className="rd-hero-input direct host-field font-mono-tabular"
                    value={host}
                    autoComplete="off"
                    spellCheck={false}
                    placeholder={t(locale, 'directTargetPlaceholder')}
                    onChange={(e) => {
                      setHeroInputError(false);
                      onChangeField('host', e.target.value);
                    }}
                  />
                  <span className="text-[var(--ink-soft)] font-mono-tabular shrink-0">:</span>
                  <input
                    type="text"
                    inputMode="numeric"
                    data-testid="port"
                    className="rd-hero-input direct port-field font-mono-tabular"
                    value={port}
                    aria-label={t(locale, 'directPortLabel')}
                    placeholder="21118"
                    onChange={(e) => onChangeField('port', e.target.value)}
                  />
                </div>
                <button
                  type="submit"
                  className="rd-hero-action-btn primary-arrow"
                  data-testid="connect"
                  aria-label={t(locale, 'connect')}
                  data-tooltip={t(locale, 'connect')}
                  data-tooltip-pos="top-end"
                  disabled={isConnecting}
                >
                  <ArrowRight size={24} />
                </button>
              </div>
            )}

            {heroInputError && (
              <p
                id="rustdesk-hero-error"
                className="rd-hero-err-text"
                data-testid="hero-input-error"
                role="alert"
              >
                {activeMode === 'id' ? t(locale, 'errMissingPeerId') : t(locale, 'errMissingHost')}
              </p>
            )}
          </form>

          {(isConnecting || isLive) && (
            <div className="mt-2.5 flex items-center justify-between gap-2">
              <div className="flex items-center gap-2 min-w-0">
                <span className={`rd-status ${phase}`} data-testid="status">
                  {isConnecting && <Loader2 size={12} className="animate-spin mr-1" />}
                  {statusLabel(locale, phase)}
                </span>
                <p className="m-0 text-xs text-[var(--ink-soft)] truncate" data-testid="detail">
                  {detail}
                </p>
              </div>
              <button
                type="button"
                className={isLive ? 'rd-btn danger' : 'rd-btn'}
                style={{ minHeight: 32, padding: '4px 10px', fontSize: 12 }}
                data-testid={isLive ? 'disconnect' : 'cancel-connect'}
                onClick={onDisconnect}
              >
                {isLive ? t(locale, 'disconnect') : t(locale, 'cancel')}
              </button>
            </div>
          )}

          {!isLive && !isConnecting && (phase === 'error' || decoderError) && (
            <ErrorBanner
              locale={locale}
              rawError={decoderError || detail}
              onDismiss={onDismissError}
              onOpenNetworkSettings={
                activeMode === 'id' ? () => openNetworkServerSetup(false) : undefined
              }
              onRetryPassword={() => {
                setPendingMode(activeMode);
                setPasswordPromptOpen(true);
              }}
            />
          )}
        </section>

        <div className="rd-home-content">
          {/* Sub-Tab Bar */}
          <div className="rd-subtab-bar" data-testid="home-subtab-bar">
            <div className="flex items-center gap-1" role="tablist" aria-label={t(locale, 'recentSessions')}>
              <button
                type="button"
                role="tab"
                aria-selected={listTab === 'recent'}
                aria-label={t(locale, 'recentSessions')}
                data-tooltip={t(locale, 'recentSessions')}
                data-tooltip-pos="bottom-start"
                data-testid="mobile-subtab-recent"
                className={`rd-subtab-btn${listTab === 'recent' ? ' active' : ''}`}
                onClick={() => setListTab('recent')}
              >
                <Clock size={18} />
                <span className="hidden sm:inline">{t(locale, 'recentSessions')}</span>
              </button>
              <button
                type="button"
                role="tab"
                aria-selected={listTab === 'favorites'}
                aria-label={t(locale, 'tabFavorites')}
                data-tooltip={t(locale, 'tabFavorites')}
                data-tooltip-pos="bottom"
                data-testid="mobile-subtab-favorites"
                className={`rd-subtab-btn${listTab === 'favorites' ? ' active' : ''}`}
                onClick={() => setListTab('favorites')}
              >
                <Star size={18} />
                <span className="hidden sm:inline">{t(locale, 'tabFavorites')}</span>
              </button>
            </div>

            <div className="flex items-center gap-0.5">
              <button
                type="button"
                className={`rd-subtab-btn${searchOpen ? ' active' : ''}`}
                aria-label={t(locale, 'searchDevicesAria')}
                aria-expanded={searchOpen}
                data-tooltip={t(locale, 'searchDevicesAria')}
                data-tooltip-pos="bottom-end"
                data-testid="mobile-search-toggle"
                onClick={() => {
                  setSearchOpen((prev) => {
                    if (prev) setSearchQuery('');
                    return !prev;
                  });
                }}
              >
                <Search size={18} />
              </button>

              {isDesktop && (
                <button
                  type="button"
                  className="rd-subtab-btn"
                  data-testid="toggle-device-layout"
                  aria-label={
                    viewLayout === 'grid' ? t(locale, 'listViewAria') : t(locale, 'gridViewAria')
                  }
                  data-tooltip={
                    viewLayout === 'grid' ? t(locale, 'listViewAria') : t(locale, 'gridViewAria')
                  }
                  data-tooltip-pos="bottom-end"
                  onClick={() => setViewLayout((prev) => (prev === 'grid' ? 'list' : 'grid'))}
                >
                  {viewLayout === 'grid' ? <LayoutGrid size={18} /> : <List size={18} />}
                </button>
              )}
            </div>
          </div>

          {/* Inline Search Bar */}
          {searchOpen && (
            <div className="px-4 md:px-0 pt-3 pb-1">
              <div className="rd-search-bar">
                <Search size={16} className="text-[var(--ink-soft)] shrink-0" />
                <input
                  type="text"
                  className="w-full bg-transparent border-0 outline-none text-sm text-[var(--ink)]"
                  data-testid="mobile-search-input"
                  value={searchQuery}
                  placeholder={t(locale, 'searchDevicesPlaceholder')}
                  onChange={(e) => setSearchQuery(e.target.value)}
                />
                {searchQuery && (
                  <button
                    type="button"
                    className="inline-flex items-center justify-center w-8 h-8 border-0 bg-transparent text-[var(--ink-soft)]"
                    aria-label={t(locale, 'clearInput')}
                    data-tooltip={t(locale, 'clearInput')}
                    data-tooltip-pos="top-end"
                    onClick={() => setSearchQuery('')}
                  >
                    <X size={16} />
                  </button>
                )}
              </div>
            </div>
          )}

          {/* Peer Cards Grid / List */}
          <section className="flex-1 flex flex-col px-4 md:px-0 py-4">
            <PeerList
              locale={locale}
              devices={filteredDevices}
              listTab={listTab}
              searchQuery={searchQuery}
              layout={effectiveLayout}
              onSelect={handleDeviceRowTap}
              onToggleFavorite={onToggleFavorite}
              onRemove={onRemoveDevice}
            />
          </section>
        </div>
      </main>

      {passwordPromptOpen && (
        <PasswordModal
          locale={locale}
          targetLabel={activeTargetLabel}
          password={password}
          rememberPassword={rememberPassword}
          hasSavedPassword={Boolean(getSavedPassword(savedPasswords, pendingDeviceKey))}
          gatewayToken={gatewayToken}
          authRequired={authRequired}
          isConnecting={isConnecting}
          onChangePassword={(val) => onChangeField('password', val)}
          onChangeRememberPassword={onChangeRememberPassword}
          onClearSavedPassword={() => {
            onClearSavedPassword(pendingDeviceKey);
            onChangeField('password', '');
          }}
          onChangeGatewayToken={(val) => onChangeField('gatewayToken', val)}
          onSubmit={handleConfirmPasswordConnect}
          onClose={() => setPasswordPromptOpen(false)}
        />
      )}
    </div>
  );
}
