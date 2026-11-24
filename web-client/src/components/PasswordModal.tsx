import React from 'react';
import { X } from 'lucide-react';
import { t, type Locale } from '../lib/i18n';

interface PasswordModalProps {
  locale: Locale;
  targetLabel: string;
  password: string;
  gatewayToken: string;
  authRequired: boolean;
  isConnecting: boolean;
  onChangePassword: (value: string) => void;
  onChangeGatewayToken: (value: string) => void;
  onSubmit: (e: React.FormEvent) => void;
  onClose: () => void;
}

export function PasswordModal({
  locale,
  targetLabel,
  password,
  gatewayToken,
  authRequired,
  isConnecting,
  onChangePassword,
  onChangeGatewayToken,
  onSubmit,
  onClose,
}: PasswordModalProps) {
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
        data-testid="password-modal"
        onSubmit={onSubmit}
      >
        <div className="flex items-center justify-between gap-2 mb-1.5">
          <h2 className="text-base font-semibold text-[var(--ink)] m-0">
            {t(locale, 'passwordPromptTitle')}
          </h2>
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

        <p className="mt-0 mb-3 text-xs text-[var(--ink-soft)]">
          {t(locale, 'passwordPromptSubtitle')}{' '}
          <span className="font-mono-tabular font-semibold text-[var(--ink)]">{targetLabel}</span>
        </p>

        <div className="flex flex-col gap-3">
          <label className="flex flex-col gap-1 text-xs font-medium text-[var(--ink-soft)]">
            <span>{t(locale, 'password')}</span>
            <input
              type="password"
              className="rd-input"
              data-testid="password"
              value={password}
              autoFocus
              autoComplete="current-password"
              placeholder={t(locale, 'passwordPlaceholder')}
              onChange={(e) => onChangePassword(e.target.value)}
            />
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
                onChange={(e) => onChangeGatewayToken(e.target.value)}
              />
            </label>
          )}
        </div>

        <div className="flex items-center justify-end gap-2 mt-4">
          <button
            type="button"
            className="rd-btn"
            style={{ minHeight: 44 }}
            onClick={onClose}
          >
            {t(locale, 'cancel')}
          </button>
          <button
            type="submit"
            className="rd-btn primary flex-1"
            style={{ minHeight: 44 }}
            data-testid="password-confirm-connect"
            disabled={isConnecting}
          >
            {isConnecting ? t(locale, 'connecting') : t(locale, 'confirmConnect')}
          </button>
        </div>
      </form>
    </div>
  );
}
