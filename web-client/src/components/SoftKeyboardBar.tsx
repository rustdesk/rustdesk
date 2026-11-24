import React, { useEffect, useRef, useState } from 'react';
import { ChevronDown } from 'lucide-react';
import { keyboardInset } from '../lib/gestures';
import { t, type Locale } from '../lib/i18n';
import {
  buildSoftKeyChord,
  controlFromKeyOrCode,
  diffTextInput,
  isImeEvent,
  type SoftChordPart,
} from '../lib/keys';

interface SoftKeyboardBarProps {
  locale: Locale;
  inputRef: React.RefObject<HTMLInputElement | null>;
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
  inputRef,
  onSendText,
  onSendControl,
  onSendChord,
  onHide,
}: SoftKeyboardBarProps) {
  const composingRef = useRef(false);
  const lastSentRef = useRef('');
  const [latched, setLatched] = useState<string[]>([]);

  useEffect(() => {
    const viewport = window.visualViewport;
    if (!viewport) return;

    let keyboardWasVisible = false;
    const syncKeyboard = () => {
      const inset = keyboardInset(window.innerHeight, viewport.height, viewport.offsetTop);
      if (inset > 80) {
        keyboardWasVisible = true;
      } else if (keyboardWasVisible && inset < 24) {
        keyboardWasVisible = false;
        onHide();
      }
    };

    syncKeyboard();
    viewport.addEventListener('resize', syncKeyboard);
    viewport.addEventListener('scroll', syncKeyboard);
    return () => {
      viewport.removeEventListener('resize', syncKeyboard);
      viewport.removeEventListener('scroll', syncKeyboard);
    };
  }, [onHide]);

  const resetLocalDraft = () => {
    lastSentRef.current = '';
    if (inputRef.current) {
      inputRef.current.value = '';
    }
  };

  const toggleModifier = (control: string) => {
    setLatched((prev) =>
      prev.includes(control) ? prev.filter((m) => m !== control) : [...prev, control],
    );
    inputRef.current?.focus({ preventScroll: true });
  };

  const tapExtraKey = (control: string) => {
    if (latched.length > 0) {
      onSendChord(buildSoftKeyChord(latched, { control }));
      setLatched([]);
    } else {
      onSendControl('down', control);
      onSendControl('up', control);
    }
    inputRef.current?.focus({ preventScroll: true });
  };

  const syncInputValue = (el: HTMLInputElement) => {
    const nextVal = el.value;
    const prevVal = lastSentRef.current;

    if (latched.length > 0) {
      const diff = diffTextInput(prevVal, nextVal);
      if (diff.insert) {
        const chars = Array.from(diff.insert);
        const first = chars.shift();
        if (first) {
          onSendChord(buildSoftKeyChord(latched, { chr: first.toLowerCase() }));
        }
        setLatched([]);
        // Restore local draft without leaving the shortcut character behind
        el.value = prevVal;
        if (chars.length > 0) {
          onSendText(chars.join(''));
        }
        return;
      }
    }

    const diff = diffTextInput(prevVal, nextVal);
    if (diff.backspaces === 0 && !diff.insert) {
      return;
    }

    for (let i = 0; i < diff.backspaces; i += 1) {
      onSendControl('down', 'Backspace');
      onSendControl('up', 'Backspace');
    }
    if (diff.insert) {
      onSendText(diff.insert);
    }

    lastSentRef.current = nextVal;
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (
      composingRef.current ||
      isImeEvent({
        isComposing: e.nativeEvent.isComposing,
        key: e.key,
        keyCode: e.keyCode,
      })
    ) {
      return;
    }

    const control = controlFromKeyOrCode(e.code, e.key);
    if (!control || control === 'Space') return;

    // When the local buffer has text and no modifiers are latched, let native Backspace
    // edit the input buffer and forward the exact diff via handleInput.
    if (control === 'Backspace' && latched.length === 0 && e.currentTarget.value.length > 0) {
      return;
    }

    e.preventDefault();
    if (latched.length > 0) {
      onSendChord(buildSoftKeyChord(latched, { control }));
      setLatched([]);
    } else {
      onSendControl('down', control);
    }

    if (control === 'Enter') {
      resetLocalDraft();
    }
  };

  const handleKeyUp = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (
      composingRef.current ||
      isImeEvent({
        isComposing: e.nativeEvent.isComposing,
        key: e.key,
        keyCode: e.keyCode,
      })
    ) {
      return;
    }
    const control = controlFromKeyOrCode(e.code, e.key);
    if (control && control !== 'Space' && latched.length === 0) {
      if (control === 'Backspace' && e.currentTarget.value.length > 0) {
        return;
      }
      e.preventDefault();
      onSendControl('up', control);
    }
  };

  const handleCompositionStart = () => {
    composingRef.current = true;
  };

  const handleCompositionEnd = (e: React.CompositionEvent<HTMLInputElement>) => {
    composingRef.current = false;
    syncInputValue(e.currentTarget);
  };

  const handleInput = (e: React.FormEvent<HTMLInputElement>) => {
    if (composingRef.current) return;
    syncInputValue(e.currentTarget);
  };

  return (
    <div
      className="softbar"
      data-testid="softbar"
      onPointerDown={(e) => e.stopPropagation()}
      onMouseDown={(e) => {
        if (e.target instanceof Element && e.target.closest('button')) {
          e.preventDefault();
        }
      }}
    >
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

      <button
        type="button"
        className="soft-key-btn"
        data-testid="soft-hide"
        aria-label={t(locale, 'softHide')}
        onClick={onHide}
      >
        <ChevronDown size={18} aria-hidden="true" />
      </button>
      <input
        ref={inputRef}
        type="text"
        className="soft-key-capture"
        data-testid="soft-input"
        aria-label={t(locale, 'softPlaceholder')}
        tabIndex={-1}
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
    </div>
  );
}
