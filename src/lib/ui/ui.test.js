// Empty, Loading and Error are THREE DIFFERENT FACTS, and the app used to conflate them.
//
// "No rows on screen" can mean:
//   - we asked, and there is nothing here          → EmptyState
//   - we have not finished asking                  → Loading
//   - asking failed                                → ErrorState
//
// Live rendered the first when it meant the second, so an operator with a full plan
// library was told "No service plans yet" on every visit — the one message that makes a
// new operator think they have lost their work. These tests pin the distinction, and
// the accessibility semantics that go with each.
import { describe, it, expect, afterEach, vi } from 'vitest';
import EmptyState from './EmptyState.svelte';
import Loading from './Loading.svelte';
import ErrorState from './ErrorState.svelte';

let host;
function mount(Component, props = {}) {
  host = document.createElement('div');
  document.body.appendChild(host);
  return new Component({ target: host, props });
}
afterEach(() => {
  host?.remove();
  host = null;
});

describe('EmptyState', () => {
  it('says what is missing', () => {
    mount(EmptyState, { message: 'No service plans yet.' });
    expect(host.textContent).toContain('No service plans yet.');
  });

  // An empty list is NOT news. It is already read out when the operator navigates to
  // it, and announcing it again would talk over them for nothing.
  it('is NOT announced — an empty list is not an event', () => {
    mount(EmptyState, { message: 'Nothing here.' });
    expect(host.querySelector('[role="status"]')).toBe(null);
    expect(host.querySelector('[role="alert"]')).toBe(null);
    expect(host.querySelector('[aria-live]')).toBe(null);
  });
});

describe('Loading', () => {
  it('names what it is fetching', () => {
    mount(Loading, { what: 'plans' });
    expect(host.textContent).toContain('Loading plans…');
  });

  // A sighted operator sees the word. A screen-reader operator was told nothing, so a
  // slow query was indistinguishable from a dead button.
  it('IS announced, politely, and marks the region busy', () => {
    mount(Loading, { what: 'services' });
    const el = host.querySelector('[role="status"]');
    expect(el).not.toBe(null);
    expect(el.getAttribute('aria-live')).toBe('polite'); // never talks over the operator
    expect(el.getAttribute('aria-busy')).toBe('true');
  });
});

describe('ErrorState', () => {
  it('renders nothing at all when there is no error', () => {
    mount(ErrorState, { error: null });
    expect(host.textContent.trim()).toBe('');
  });

  // The only one of the three that interrupts. An operator acting on a command that
  // silently failed is about to make it worse.
  it('IS assertive — it interrupts', () => {
    mount(ErrorState, { error: { kind: 'internal', message: 'boom' } });
    expect(host.querySelector('[role="alert"]')).not.toBe(null);
  });

  it('humanises the error rather than dumping the Rust string', () => {
    mount(ErrorState, {
      error: { kind: 'io', message: 'failed to bind 0.0.0.0:8032: Address already in use' },
    });
    expect(host.textContent).toMatch(/second copy of Relay/i);
    expect(host.textContent).not.toContain('0.0.0.0'); // no addresses at a volunteer
  });

  // THE POINT OF TYPING THE ERRORS.
  //
  // A "Try again" button that cannot possibly work is worse than no button: the
  // operator presses it instead of fixing the actual problem. The backend now says
  // whether the fault is transient, so the button only appears when it might help.
  it('offers Try again for a BUSY database — retrying genuinely works', () => {
    mount(ErrorState, { error: { kind: 'busy', message: 'Relay is busy saving.' }, onRetry: vi.fn() });
    expect(host.querySelector('button')).not.toBe(null);
  });

  it('does NOT offer Try again for a full disk — pressing it again will never work', () => {
    mount(ErrorState, {
      error: { kind: 'io', message: 'No space left on device' },
      onRetry: vi.fn(),
    });
    expect(host.querySelector('button')).toBe(null);
  });

  it('does not offer Try again when the caller gave no way to retry', () => {
    mount(ErrorState, { error: { kind: 'busy', message: 'busy' } });
    expect(host.querySelector('button')).toBe(null);
  });

  it('the retry button actually calls back', async () => {
    const onRetry = vi.fn();
    mount(ErrorState, { error: { kind: 'busy', message: 'busy' }, onRetry });
    host.querySelector('button').click();
    expect(onRetry).toHaveBeenCalledTimes(1);
  });
});
