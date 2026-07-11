import { describe, it, expect, beforeEach, vi } from 'vitest';
import { installCrashGuard, installLeaveGuard } from './crash.js';

const panel = () => document.getElementById('relay-crash-panel');

describe('crash guard', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
    localStorage.clear();
    installCrashGuard();
  });

  it('renders a recovery panel instead of leaving a white screen', () => {
    window.dispatchEvent(
      new ErrorEvent('error', { error: new Error('boom'), message: 'boom' }),
    );
    expect(panel()).not.toBeNull();
  });

  // The single most important thing this panel does. An operator who knows the
  // congregation sees nothing does not panic.
  it('tells the operator the output screens are still live', () => {
    window.dispatchEvent(new ErrorEvent('error', { error: new Error('boom') }));
    expect(panel().textContent).toMatch(/output screens are still live/i);
    expect(panel().textContent).toMatch(/congregation sees no interruption/i);
  });

  it('offers a recover action', () => {
    window.dispatchEvent(new ErrorEvent('error', { error: new Error('boom') }));
    expect(panel().querySelector('#relay-crash-recover')).not.toBeNull();
  });

  it('catches unhandled promise rejections too', () => {
    const e = new Event('unhandledrejection');
    e.reason = new Error('async boom');
    window.dispatchEvent(e);
    expect(panel()).not.toBeNull();
  });

  it('does not stack panels when errors cascade', () => {
    for (let i = 0; i < 5; i++) {
      window.dispatchEvent(new ErrorEvent('error', { error: new Error(`boom ${i}`) }));
    }
    expect(document.querySelectorAll('#relay-crash-panel')).toHaveLength(1);
  });

  it('ignores a failed image load — a missing asset is not a crash', () => {
    const img = document.createElement('img');
    document.body.appendChild(img);
    const e = new Event('error', { bubbles: true });
    Object.defineProperty(e, 'target', { value: img });
    window.dispatchEvent(e);
    expect(panel()).toBeNull();
  });

  // The error text is attacker-influenced (it can contain imported content), and
  // this panel must not become an injection vector on its way to being helpful.
  it('renders the error as text, never as HTML', () => {
    const err = new Error('<img src=x onerror="alert(1)">');
    window.dispatchEvent(new ErrorEvent('error', { error: err }));
    const pre = panel().querySelector('pre');
    expect(pre.querySelector('img')).toBeNull();
    expect(pre.textContent).toContain('<img src=x');
  });

  it('shows where the operator will resume to', () => {
    localStorage.setItem(
      'relay.session.v1',
      JSON.stringify({ activeTab: 'planner', planId: 7, liveCueId: 3, liveSlide: 1 }),
    );
    window.dispatchEvent(new ErrorEvent('error', { error: new Error('boom') }));
    const t = panel().textContent;
    expect(t).toContain('plan #7');
    expect(t).toContain('cue #3');
    expect(t).toContain('slide 2'); // 0-indexed internally, 1-indexed for humans
  });

  it('survives a corrupt session payload rather than failing to render', () => {
    localStorage.setItem('relay.session.v1', 'not json{{{');
    expect(() =>
      window.dispatchEvent(new ErrorEvent('error', { error: new Error('boom') })),
    ).not.toThrow();
    expect(panel()).not.toBeNull();
  });
});

describe('leave guard', () => {
  it('warns before unloading while the mic is live', () => {
    const stop = installLeaveGuard(() => true);
    const e = new Event('beforeunload', { cancelable: true });
    window.dispatchEvent(e);
    expect(e.defaultPrevented).toBe(true);
    stop();
  });

  it('does not warn when not capturing', () => {
    const stop = installLeaveGuard(() => false);
    const e = new Event('beforeunload', { cancelable: true });
    window.dispatchEvent(e);
    expect(e.defaultPrevented).toBe(false);
    stop();
  });
});
