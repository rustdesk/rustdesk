import React, { useState } from 'react';
import { Activity, Clipboard, Command, Eye, Focus, LockKeyhole, LogOut, ScanEye, Monitor, MousePointer2, Pointer, RefreshCw, Scaling, SlidersHorizontal, Volume2, CircleHelp } from 'lucide-react';
import { t, type I18nKey } from '../lib/i18n';
import { formatConnectionError } from '../lib/errors';
import type { SessionMenuProps } from './SessionMenu';
import type { PreferredCodec } from '../lib/displayPrefs';
const qualityNames = { low: 'qualityLow', balanced: 'qualityBalanced', best: 'qualityBest' } as const;

export type MobilePanel = 'toolbar' | 'actions' | 'input' | 'display';

const gestureRows = {
  touch: [
    ['gTouchTapTitle', 'gTouchTapDesc'],
    ['gDoubleTapTitle', 'gTouchDoubleTapDesc'],
    ['gTouchRightTitle', 'gTouchRightDesc'],
    ['gTouchDragTitle', 'gTouchDragDesc'],
    ['gTouchPinchTitle', 'gTouchPinchDesc'],
    ['gTouchScrollTitle', 'gTouchScrollDesc'],
  ],
  mouse: [
    ['gMouseSlideTitle', 'gMouseSlideDesc'],
    ['gMouseTapTitle', 'gMouseTapDesc'],
    ['gDoubleTapTitle', 'gMouseDoubleTapDesc'],
    ['gMouseDragTitle', 'gMouseDragDesc'],
    ['gMouseRightTitle', 'gMouseRightDesc'],
    ['gMousePinchTitle', 'gMousePinchDesc'],
    ['gTouchScrollTitle', 'gTouchScrollDesc'],
  ],
} as const;

export function MobileSessionPanel(p: SessionMenuProps & { mobilePanel: MobilePanel }) {
  const [draft, setDraft] = useState('');
  const [notice, setNotice] = useState('');
  const label = (key: I18nKey) => t(p.locale, key);
  const monitors = p.peer?.displays ?? [];
  const panelName = { toolbar: 'session', actions: 'moreActions', input: 'pointerSettings', display: 'screens' } as const;

  const toggle = (Icon: typeof Eye, text: I18nKey, checked: boolean, change: () => void, disabled = false) => (
    <label className="tool-row"><Icon /><span>{label(text)}</span>
      <input type="checkbox" role="switch" checked={checked} onChange={change} disabled={disabled} />
    </label>
  );
  const action = (Icon: typeof Eye, text: I18nKey, run: () => void, danger = false) => (
    <button type="button" className={danger ? 'tool-row destructive' : 'tool-row'} onClick={run}><Icon />{label(text)}</button>
  );
  return (
    <div className="remote-menu-backdrop" data-testid="session-backdrop" onPointerDown={e => e.stopPropagation()}
      onClick={e => { if (e.target === e.currentTarget) p.onClose(); }}>
      {p.mobilePanel !== 'toolbar' && (
        <section className="remote-menu-panel" role="dialog" aria-label={label(panelName[p.mobilePanel])} data-testid="session-menu">
          {p.mobilePanel === 'actions' && <>
            <div className="tool-group">
              <details><summary><Command />{label('shortcutKeys')}</summary>
                <div className="tool-options">{(['Ctrl+C', 'Ctrl+V', 'Ctrl+Alt+Del'] as const).map(k => <button key={k} onClick={() => p.onQuickKey(k)}>{k}</button>)}</div>
              </details>
              {action(LockKeyhole, 'lockScreen', () => p.onQuickKey('Lock'))}
              <details><summary><Clipboard />{label('clipboard')}</summary>
                <form className="tool-options" onSubmit={e => {
                  e.preventDefault();
                  if (!draft.trim() || p.clipboardDisabled) return;
                  p.onSendClipboard(draft); setDraft(''); setNotice(label('clipSent'));
                }}>
                  <input type="text" data-testid="clipboard-input" aria-label={label('clipboard')} value={draft}
                    placeholder={label('clipboardPlaceholder')} disabled={p.clipboardDisabled} onChange={e => setDraft(e.target.value)} />
                  <button disabled={!draft.trim() || p.clipboardDisabled}>{label('send')}</button>
                </form>
                {p.remoteClipboard && <button className="tool-row" disabled={p.clipboardDisabled} onClick={async () => {
                  try { await navigator.clipboard.writeText(p.remoteClipboard); setNotice(label('clipCopied')); }
                  catch { setNotice(label('clipDenied')); }
                }}>{label('copy')} · {p.remoteClipboard.slice(0, 40)}</button>}
                {notice && <p role="status">{notice}</p>}
              </details>
            </div>
            <div className="tool-group">
              {toggle(Eye, 'optViewOnly', p.viewOnly, p.onToggleViewOnly)}
              {toggle(Volume2, 'remoteAudio', p.audioEnabled, p.onToggleAudio)}
              {p.audioIssue && <p role="alert">{p.audioIssue}</p>}
            </div>
            <div className="tool-group">{action(Activity, 'diagTitle', p.onToggleDiagnostics)}</div>
            <div className="tool-group">{action(LogOut, 'disconnectRemote', p.onDisconnect, true)}</div>
          </>}
          {p.mobilePanel === 'input' && <>
            <div className="tool-group tool-options mode-options">
              {([['touch', Pointer, true], ['mouse', MousePointer2, false]] as const).map(([name, Icon, touch]) =>
                <button key={name} aria-pressed={p.touchMode === touch} onClick={() => { if (p.touchMode !== touch) p.onToggleTouchMode(); }}><Icon />{label(name)}</button>)}
            </div>
            <div className="tool-group">
              {toggle(MousePointer2, 'optShowRemoteCursor', p.showRemoteCursor, p.onToggleRemoteCursor)}
              {toggle(Scaling, 'enlargeCursor', p.enlargeCursor, p.onToggleEnlargeCursor, !p.showRemoteCursor || p.cursorEmbedded)}
              {toggle(Focus, 'followCursor', p.followRemoteCursor, p.onToggleFollowCursor)}
            </div>
            <div className="tool-group"><details><summary><CircleHelp />{label('dockGuide')}</summary>
              <dl data-testid="gesture-guide">{gestureRows[p.touchMode ? 'touch' : 'mouse'].map(([name, desc]) =>
                <div key={name}><dt>{label(name)}</dt><dd>{label(desc)}</dd></div>)}</dl>
            </details></div>
          </>}
          {p.mobilePanel === 'display' && <>
            {monitors.length > 1 && <div className="tool-group tool-options">{monitors.map(d =>
              <button key={d.index} aria-pressed={d.index === p.display} disabled={d.online === false}
                onClick={() => p.onSwitchDisplay(d.index)}><Monitor />{d.name || label('screens')+' '+(d.index+1)}</button>)}</div>}
            <div className="tool-group">
              <details><summary><ScanEye />{label('view')}<small>{p.viewMode === 'fit' ? label('fit') : p.zoomPercent+'%'}</small></summary>
                <div className="tool-options">
                  <button data-testid="view-fit" onClick={p.onFit}>{label('fit')}</button><button onClick={p.onOriginal}>100%</button>
                  <button aria-label={label('zoomOut')} onClick={p.onZoomOut}>−</button>{p.zoomPercent}%
                  <button aria-label={label('zoomIn')} onClick={p.onZoomIn}>＋</button>
                  <button aria-pressed={p.hand} onClick={p.onToggleHand}>{label('pan')}</button>
                </div>
              </details>
              {p.sizes.length > 0 && <label className="tool-row"><Monitor /><span>{label('remoteResolution')}</span>
                <select disabled={p.viewOnly} value={p.sizes.findIndex(s => s.width === p.stats.width && s.height === p.stats.height)}
                  onChange={e => { const size = p.sizes[Number(e.target.value)]; if (size) p.onResolution(size.width,size.height); }}>
                  <option disabled value={-1}>{p.stats.width} × {p.stats.height}</option>
                  {p.sizes.map((s,i) => <option key={i} value={i}>{s.width} × {s.height}</option>)}
                </select></label>}
              <details data-testid="mobile-quality">
                <summary><SlidersHorizontal />{label('qualityAndFps')}<small>{label(qualityNames[p.quality])} · {p.targetFps} FPS</small></summary>
                <div className="tool-options mode-options">
                  {(['low', 'balanced', 'best'] as const).map(level =>
                    <button key={level} aria-pressed={p.quality === level} onClick={() => p.onChangeQuality(level, p.targetFps)}>
                      {label(qualityNames[level])}
                    </button>)}
                </div>
                <label className="tool-row"><span>{label('targetFps')}</span><select value={p.targetFps} onChange={e => p.onChangeQuality(p.quality, Number(e.target.value))}>
                  {[15,30,60].map(n => <option key={n}>{n}</option>)}</select></label>
                <label className="tool-row"><span>{label('sessionCodecLabel')}</span><select value={p.preferredCodec} onChange={e => p.onChangeQuality(p.quality,p.targetFps,e.target.value as PreferredCodec)}>
                  {['auto','h264','vp8','vp9','av1'].map(v => <option key={v} value={v}>{v==='auto'?label('codecAuto'):v.toUpperCase()}</option>)}</select></label>
              </details>
            </div>
            <div className="tool-group">
              {monitors.length > 1 && toggle(Focus,'followWindow',p.followWindow,p.onToggleFollowWindow)}
              {action(RefreshCw,'refresh',p.onRefresh)}
            </div>
          </>}
          {p.decoderError && <p role="alert">{formatConnectionError(p.locale,p.decoderError).message}</p>}
        </section>
      )}
    </div>
  );
}
