import React from 'react';
import { Globe } from 'lucide-react';
import { statusLabel, t, type Locale } from '../lib/i18n';

export type ConnectionMode = 'direct' | 'id' | 'demo';
export type ConnectionPhase = 'idle' | 'connecting' | 'live' | 'error';

interface ConnectionSheetProps {
  locale: Locale;
  onToggleLocale: () => void;
  mode: ConnectionMode;
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
  onChangeMode: (mode: ConnectionMode) => void;
  onChangeField: (
    field:
      | 'host'
      | 'port'
      | 'password'
      | 'peerId'
      | 'idServer'
      | 'relayServer'
      | 'serverKey'
      | 'gatewayToken',
    value: string,
  ) => void;
  onChangeForceRelay: (checked: boolean) => void;
  onConnect: () => void;
  onDisconnect: () => void;
  onClose: () => void;
}

export function ConnectionSheet({
  locale,
  onToggleLocale,
  mode,
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
  onChangeMode,
  onChangeField,
  onChangeForceRelay,
  onConnect,
  onDisconnect,
  onClose,
}: ConnectionSheetProps) {
  const isLive = phase === 'live';
  const isConnecting = phase === 'connecting';

  const submitLabel = isConnecting
    ? t(locale, 'connecting')
    : phase === 'error' || isLive
      ? t(locale, 'reconnect')
      : t(locale, 'connect');

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    onConnect();
  };

  return (
    <div
      className="scrim"
      onPointerDown={(e) => {
        e.stopPropagation();
        if (e.target === e.currentTarget && isLive) {
          onClose();
        }
      }}
    >
      <form
        className="sheet"
        data-testid="connection-panel"
        onSubmit={handleSubmit}
        onPointerDown={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between gap-2.5 mb-3">
          <div className="flex items-center gap-2.5 min-w-0">
            <h1 className="text-base font-semibold text-[#1c2430] m-0 whitespace-nowrap">
              {t(locale, 'connection')}
            </h1>
            <span className={`rd-status ${phase}`} data-testid="status">
              {statusLabel(locale, phase)}
            </span>
          </div>
          <div className="flex items-center gap-2">
            <button
              type="button"
              className="rd-btn"
              data-testid="lang-toggle"
              aria-label={t(locale, 'langToggleAria')}
              onClick={onToggleLocale}
            >
              <Globe size={15} />
              <span>{t(locale, 'langToggle')}</span>
            </button>
            {isLive && (
              <button type="button" className="rd-btn" onClick={onClose}>
                {t(locale, 'close')}
              </button>
            )}
          </div>
        </div>

        {/* Mode segmented buttons */}
        <div
          className="grid grid-cols-3 gap-2 mb-4"
          role="tablist"
          aria-label={t(locale, 'connectionModeAria')}
        >
          <button
            type="button"
            role="tab"
            aria-selected={mode === 'direct'}
            data-testid="mode-direct"
            className={`rd-btn${mode === 'direct' ? ' active' : ''}`}
            onClick={() => onChangeMode('direct')}
          >
            {t(locale, 'modeDirect')}
          </button>
          <button
            type="button"
            role="tab"
            aria-selected={mode === 'id'}
            data-testid="mode-id"
            className={`rd-btn${mode === 'id' ? ' active' : ''}`}
            onClick={() => onChangeMode('id')}
          >
            {t(locale, 'modeId')}
          </button>
          <button
            type="button"
            role="tab"
            aria-selected={mode === 'demo'}
            data-testid="mode-demo"
            className={`rd-btn${mode === 'demo' ? ' active' : ''}`}
            onClick={() => onChangeMode('demo')}
          >
            {t(locale, 'modeDemo')}
          </button>
        </div>

        {/* Mode-specific fields */}
        <div className="flex flex-col gap-3">
          {mode === 'direct' && (
            <div className="grid grid-cols-[1fr_104px] gap-2.5">
              <label className="flex flex-col gap-1 text-xs font-medium text-[#5c6b7a]">
                <span>{t(locale, 'host')}</span>
                <input
                  type="text"
                  className="rd-input font-mono-tabular"
                  data-testid="host"
                  value={host}
                  autoComplete="off"
                  spellCheck={false}
                  placeholder="127.0.0.1"
                  onChange={(e) => onChangeField('host', e.target.value)}
                />
              </label>
              <label className="flex flex-col gap-1 text-xs font-medium text-[#5c6b7a]">
                <span>{t(locale, 'port')}</span>
                <input
                  type="text"
                  inputMode="numeric"
                  className="rd-input font-mono-tabular"
                  data-testid="port"
                  value={port}
                  placeholder="21118"
                  onChange={(e) => onChangeField('port', e.target.value)}
                />
              </label>
            </div>
          )}

          {mode === 'id' && (
            <>
              <div className="grid grid-cols-1 sm:grid-cols-2 gap-2.5">
                <label className="flex flex-col gap-1 text-xs font-medium text-[#5c6b7a]">
                  <span>{t(locale, 'peerId')}</span>
                  <input
                    type="text"
                    className="rd-input font-mono-tabular"
                    data-testid="peer-id"
                    value={peerId}
                    autoComplete="off"
                    placeholder="123456789"
                    onChange={(e) => onChangeField('peerId', e.target.value)}
                  />
                </label>
                <label className="flex flex-col gap-1 text-xs font-medium text-[#5c6b7a]">
                  <span>{t(locale, 'idServer')}</span>
                  <input
                    type="text"
                    className="rd-input font-mono-tabular"
                    data-testid="id-server"
                    value={idServer}
                    autoComplete="off"
                    placeholder="hbbs.example.com"
                    onChange={(e) => onChangeField('idServer', e.target.value)}
                  />
                </label>
              </div>

              <div className="grid grid-cols-1 sm:grid-cols-2 gap-2.5">
                <label className="flex flex-col gap-1 text-xs font-medium text-[#5c6b7a]">
                  <span>{t(locale, 'relayServer')}</span>
                  <input
                    type="text"
                    className="rd-input font-mono-tabular"
                    data-testid="relay-server"
                    value={relayServer}
                    autoComplete="off"
                    placeholder={t(locale, 'optional')}
                    onChange={(e) => onChangeField('relayServer', e.target.value)}
                  />
                </label>
                <label className="flex flex-col gap-1 text-xs font-medium text-[#5c6b7a]">
                  <span>{t(locale, 'serverPublicKey')}</span>
                  <input
                    type="text"
                    className="rd-input font-mono-tabular"
                    data-testid="server-key"
                    value={serverKey}
                    autoComplete="off"
                    placeholder={t(locale, 'base64Optional')}
                    onChange={(e) => onChangeField('serverKey', e.target.value)}
                  />
                </label>
              </div>

              <label className="flex items-center gap-2.5 min-h-[40px] text-sm text-[#1c2430] cursor-pointer select-none">
                <input
                  type="checkbox"
                  className="w-4 h-4 accent-[#0b6bcb]"
                  checked={forceRelay}
                  onChange={(e) => onChangeForceRelay(e.target.checked)}
                />
                <span>{t(locale, 'forceRelay')}</span>
              </label>
            </>
          )}

          {mode === 'demo' && (
            <div className="rounded-lg border border-[#d5dde6] bg-[#f7f9fb] px-3 py-2.5 text-xs text-[#5c6b7a] leading-relaxed">
              {t(locale, 'demoDescriptionPrefix')}{' '}
              <span className="font-mono-tabular text-[#1c2430]">/ws</span>{' '}
              {t(locale, 'demoDescriptionMid')}{' '}
              <span className="font-mono-tabular text-[#1c2430]">RDV1</span>{' '}
              {t(locale, 'demoDescriptionSuffix')}
            </div>
          )}

          {authRequired && mode !== 'demo' && (
            <label className="flex flex-col gap-1 text-xs font-medium text-[#5c6b7a]">
              <span>{t(locale, 'gatewayToken')}</span>
              <input
                type="password"
                className="rd-input"
                data-testid="gateway-token"
                value={gatewayToken}
                autoComplete="off"
                placeholder={t(locale, 'gatewayTokenPlaceholder')}
                onChange={(e) => onChangeField('gatewayToken', e.target.value)}
              />
              <span className="text-[11px] font-normal text-[#5c6b7a] leading-snug">
                {t(locale, 'gatewayTokenHint')}
              </span>
            </label>
          )}

          {mode !== 'demo' && (
            <label className="flex flex-col gap-1 text-xs font-medium text-[#5c6b7a]">
              <span>{t(locale, 'password')}</span>
              <input
                type="password"
                className="rd-input"
                data-testid="password"
                value={password}
                autoComplete="current-password"
                placeholder={t(locale, 'passwordPlaceholder')}
                onChange={(e) => onChangeField('password', e.target.value)}
              />
            </label>
          )}
        </div>

        {/* Actions */}
        <div className="flex items-center gap-2.5 mt-4">
          <button
            type="submit"
            className="rd-btn primary flex-1"
            data-testid="connect"
            disabled={isConnecting}
          >
            {submitLabel}
          </button>
          {isLive && (
            <button
              type="button"
              className="rd-btn danger"
              data-testid="disconnect"
              onClick={onDisconnect}
            >
              {t(locale, 'disconnect')}
            </button>
          )}
        </div>

        {/* Status detail & decoder warning */}
        <p
          className={`mt-3 mb-0 text-xs leading-relaxed ${
            phase === 'error' ? 'text-[#b42318] font-medium' : 'text-[#5c6b7a]'
          }`}
          data-testid="detail"
        >
          {detail}
        </p>

        {!isLive && decoderError && (
          <div
            className="mt-2.5 rounded-lg border border-[#f3c4bf] bg-[#fdecea] px-3 py-2 text-xs text-[#b42318] leading-relaxed"
            data-testid="decoder-error"
            role="alert"
          >
            {decoderError}
          </div>
        )}
      </form>
    </div>
  );
}
