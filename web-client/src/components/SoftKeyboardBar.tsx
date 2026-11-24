import React, { useEffect, useRef, useState } from 'react';
import { t, type Locale } from '../lib/i18n';
import { buildSoftKeyChord, controlFromCode, type SoftChordPart } from '../lib/keys';

interface SoftKeyboardBarProps {
  locale: Locale;
  onSendText: (text: string) => void;
  onSendControl: (action: 'down' | 'up', control: string) => void;
  onSendChord: (parts: SoftChordPart[]) => void;
  onHide: () => void;
}

const STICKY_MODIFIERS = [
  { label: 'Ctrl', control: 'Control' },
  { label: 'Alt', control: 'Alt' },
  { label: 'Shift', control: 'Shift' },
  { label: 'Win', control: 'Meta' },
] as const;

const EXTRA_KEYS = [
  { label: 'Esc', control: 'Escape' },
  { label: 'Tab', control: 'Tab' },
  { label: '←', control: 'ArrowLeft', aria: 'Left arrow' },
  { label: '↑', control: 'ArrowUp', aria: 'Up arrow' },
  { label: '↓', control: 'ArrowDown', aria: 'Down arrow' },
  { label: '→', control: 'ArrowRight', aria: 'Right arrow' },
  { label: 'Home', control: 'Home' },
  { label: 'End', control: 'End' },
  { label: 'Del', control: 'Delete' },
] as const;

export function SoftKeyboardBar({
  locale,
  onSendText,
  onSendControl,
  onSendChord,
  onHide,
}: SoftKeyboardBarProps) {
  const inputRef = useRef<HTMLInputElement>(null);
  const composingRef = useRef(false);
  const [latched, setLatched] = useState<string[]>([]);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const toggleModifier = (control: string) => {
    setLatched((prev) =>
      prev.includes(control) ? prev.filter((m) => m !== control) : [...prev, control],
    );
    inputRef.current?.focus();
  };

  const tapExtraKey = (control: string) => {
    if (latched.length > 0) {
      onSendChord(buildSoftKeyChord(latched, { control }));
      setLatched([]);
    } else {
      onSendControl('down', control);
      onSendControl('up', control);
    }
    inputRef.current?.focus();
  };

  const dispatchTypedText = (val: string) => {
    if (!val) return;
    if (latched.length > 0) {
      const chars = Array.from(val);
      const first = chars.shift();
      if (first) {
        onSendChord(buildSoftKeyChord(latched, { chr: first.toLowerCase() }));
      }
      setLatched([]);
      if (chars.length > 0) {
        onSendText(chars.join(''));
      }
      return;
    }
    onSendText(val);
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.nativeEvent.isComposing || e.key === 'Process') return;
    const control = controlFromCode(e.code);
    if (control && control !== 'Space') {
      e.preventDefault();
      if (latched.length > 0) {
        onSendChord(buildSoftKeyChord(latched, { control }));
        setLatched([]);
      } else {
        onSendControl('down', control);
      }
    }
  };

  const handleKeyUp = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.nativeEvent.isComposing || e.key === 'Process') return;
    const control = controlFromCode(e.code);
    if (control && control !== 'Space' && latched.length === 0) {
      e.preventDefault();
      onSendControl('up', control);
    }
  };

  const handleCompositionStart = () => {
    composingRef.current = true;
  };

  const handleCompositionEnd = (e: React.CompositionEvent<HTMLInputElement>) => {
    composingRef.current = false;
    if (e.data) {
      dispatchTypedText(e.data);
    }
    e.currentTarget.value = '';
  };

  const handleInput = (e: React.FormEvent<HTMLInputElement>) => {
    if (composingRef.current) return;
    const val = e.currentTarget.value;
    if (val) {
      dispatchTypedText(val);
      e.currentTarget.value = '';
    }
  };

  return (
    <div
      className="softbar"
      data-testid="softbar"
      onPointerDown={(e) => e.stopPropagation()}
    >
      {/* RustDesk Mobile-style extra special keys & sticky modifiers row */}
      <div
        className="soft-key-strip"
        data-testid="soft-key-strip"
        role="toolbar"
        aria-label={t(locale, 'specialKeysAria')}
      >
        {STICKY_MODIFIERS.map((mod) => {
          const isActive = latched.includes(mod.control);
          return (
            <button
              key={mod.control}
              type="button"
              aria-pressed={isActive}
              data-testid={`soft-mod-${mod.label.toLowerCase()}`}
              className={`soft-key-btn${isActive ? ' active' : ''}`}
              onClick={() => toggleModifier(mod.control)}
            >
              {mod.label}
            </button>
          );
        })}
        <span className="soft-key-divider" aria-hidden="true" />
        {EXTRA_KEYS.map((item) => (
          <button
            key={item.control}
            type="button"
            aria-label={'aria' in item ? item.aria : item.label}
            className="soft-key-btn"
            onClick={() => tapExtraKey(item.control)}
          >
            {item.label}
          </button>
        ))}
      </div>

      {/* Text / IME input row */}
      <div className="flex gap-2 items-center w-full">
        <input
          ref={inputRef}
          type="text"
          className="rd-input flex-1"
          data-testid="soft-input"
          placeholder={
            latched.length > 0
              ? `${t(locale, 'softNextKeyPrefix')} ${latched.join('+')}...`
              : t(locale, 'softPlaceholder')
          }
          autoCapitalize="off"
          autoCorrect="off"
          autoComplete="off"
          spellCheck={false}
          onKeyDown={handleKeyDown}
          onKeyUp={handleKeyUp}
          onCompositionStart={handleCompositionStart}
          onCompositionEnd={handleCompositionEnd}
          onInput={handleInput}
        />
        <button type="button" className="rd-btn" data-testid="soft-hide" onClick={onHide}>
          {t(locale, 'softHide')}
        </button>
      </div>
    </div>
  );
}
