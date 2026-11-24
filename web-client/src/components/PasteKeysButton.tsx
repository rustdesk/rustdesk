import { useState } from 'react';
import { t, type Locale, type I18nKey } from '../lib/i18n';

export function PasteKeysButton({ locale, onPasteKeys, pasteDisabled, className }: {
  locale: Locale;
  onPasteKeys: () => Promise<boolean>;
  pasteDisabled: boolean;
  className: string;
}) {
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<I18nKey | null>(null);
  const paste = async () => {
    setBusy(true);
    try {
      setNotice(await onPasteKeys() ? 'keysSent' : 'emptyClip');
    } catch {
      setNotice('pasteFailed');
    } finally {
      setBusy(false);
    }
  };
  return (
    <>
      <button type="button" disabled={pasteDisabled || busy} className={className} onClick={paste}>
        {t(locale, 'pasteKeys')}
      </button>
      {notice && <span role="status" className="text-xs">{t(locale, notice)}</span>}
    </>
  );
}
