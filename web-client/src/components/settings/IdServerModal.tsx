import React from 'react';
import { Server, X } from 'lucide-react';
import { t, type Locale } from '../../lib/i18n';

export type ServerField = 'idServer' | 'relayServer' | 'serverKey' | 'gatewayToken';

interface IdServerModalProps {
  locale: Locale;
  idServer: string;
  relayServer: string;
  serverKey: string;
  forceRelay: boolean;
  gatewayToken: string;
  authRequired: boolean;
  idServerError: boolean;
  onChangeField: (field: ServerField, value: string) => void;
  onChangeForceRelay: (checked: boolean) => void;
  onClearError: () => void;
  onClose: () => void;
}

export function IdServerModal({
  locale,
  idServer,
  relayServer,
  serverKey,
  forceRelay,
  gatewayToken,
  authRequired,
  idServerError,
  onChangeField,
  onChangeForceRelay,
  onClearError,
  onClose,
}: IdServerModalProps) {
  return (
    <div
      className="rd-password-scrim"
      onPointerDown={(e) => {
        if (e.target === e.currentTarget) {
          onClose();
        }
      }}
    >
      <form
        className="rd-password-dialog"
        data-testid="id-server-modal"
        onSubmit={(e) => {
          e.preventDefault();
          onClose();
        }}
      >
        <div className="flex items-center justify-between gap-2 mb-3">
          <div className="flex items-center gap-2">
            <Server size={17} className="text-[var(--accent)]" />
            <h2 className="text-base font-semibold text-[var(--ink)] m-0">
              {t(locale, 'idRelayServerTitle')}
            </h2>
          </div>
          <button
            type="button"
            className="w-9 h-9 inline-flex items-center justify-center rounded-lg border-0 bg-transparent text-[var(--ink-soft)] hover:bg-[var(--rd-hover-bg)]"
            aria-label={t(locale, 'close')}
            data-tooltip={t(locale, 'close')}
            data-tooltip-pos="bottom-end"
            onClick={onClose}
          >
            <X size={18} />
          </button>
        </div>

        <div className="flex flex-col gap-3">
          <label className="flex flex-col gap-1 text-xs font-medium text-[var(--ink-soft)]">
            <span>{t(locale, 'idServer')}</span>
            <input
              type="text"
              className={`rd-input font-mono-tabular${idServerError ? ' has-error' : ''}`}
              data-testid="id-server"
              aria-invalid={idServerError || undefined}
              value={idServer}
              autoFocus
              autoComplete="off"
              spellCheck={false}
              placeholder="hbbs.example.com:21116"
              onChange={(e) => {
                onClearError();
                onChangeField('idServer', e.target.value);
              }}
            />
          </label>

          <label className="flex flex-col gap-1 text-xs font-medium text-[var(--ink-soft)]">
            <span>{t(locale, 'relayServer')}</span>
            <input
              type="text"
              className="rd-input font-mono-tabular"
              data-testid="relay-server"
              value={relayServer}
              autoComplete="off"
              spellCheck={false}
              placeholder={t(locale, 'optional')}
              onChange={(e) => onChangeField('relayServer', e.target.value)}
            />
          </label>

          <label className="flex flex-col gap-1 text-xs font-medium text-[var(--ink-soft)]">
            <span>{t(locale, 'serverPublicKey')}</span>
            <input
              type="text"
              className="rd-input font-mono-tabular"
              data-testid="server-key"
              value={serverKey}
              autoComplete="off"
              spellCheck={false}
              placeholder={t(locale, 'base64Optional')}
              onChange={(e) => onChangeField('serverKey', e.target.value)}
            />
          </label>

          <label className="rd-check-row">
            <input
              type="checkbox"
              checked={forceRelay}
              onChange={(e) => onChangeForceRelay(e.target.checked)}
            />
            <span>{t(locale, 'forceRelay')}</span>
          </label>

          {authRequired && (
            <label className="flex flex-col gap-1 text-xs font-medium text-[var(--ink-soft)]">
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
            </label>
          )}
        </div>

        <div className="flex items-center justify-end gap-2 mt-4">
          <button
            type="button"
            className="rd-btn"
            style={{ minHeight: 42 }}
            onClick={onClose}
          >
            {t(locale, 'cancel')}
          </button>
          <button
            type="submit"
            className="rd-btn primary"
            style={{ minHeight: 42, minWidth: 88 }}
            data-testid="id-server-save"
          >
            {t(locale, 'confirmOk')}
          </button>
        </div>
      </form>
    </div>
  );
}
