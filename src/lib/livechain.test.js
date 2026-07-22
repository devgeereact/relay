// Diagnostic: the derived `liveContent` (what the Program pane and outputs
// render) must track the `live` store. If the header shows a new verse but the
// wall stays on the old one, this chain is where it breaks.

import { describe, it, expect } from 'vitest';
import { get } from 'svelte/store';
import { live, liveContent, liveTemplateOverride } from './stores/capture.js';

describe('live → liveContent chain', () => {
  it('liveContent follows live on every set', () => {
    live.set({ reference: 'Psalms 23:1', text: 'The LORD is my shepherd', template_json: null });
    expect(get(liveContent)?.reference).toBe('Psalms 23:1');
    expect(get(liveContent)?.text).toMatch(/shepherd/);

    live.set({ reference: 'Revelation 1:1', text: 'The revelation of Jesus Christ', template_json: null });
    expect(get(liveContent)?.reference).toBe('Revelation 1:1');
    expect(get(liveContent)?.text).toMatch(/revelation of Jesus/i);
  });

  it('a live subscriber and a liveContent subscriber never disagree', () => {
    let liveRef = null;
    let contentRef = null;
    const u1 = live.subscribe((v) => (liveRef = v?.reference ?? null));
    const u2 = liveContent.subscribe((v) => (contentRef = v?.reference ?? null));

    live.set({ reference: 'John 3:16', text: 'For God so loved the world', template_json: null });
    expect(liveRef).toBe('John 3:16');
    expect(contentRef).toBe('John 3:16'); // must match — this is the frozen-pane symptom

    live.set({ reference: 'Genesis 1:1', text: 'In the beginning', template_json: null });
    expect(liveRef).toBe('Genesis 1:1');
    expect(contentRef).toBe('Genesis 1:1');

    u1();
    u2();
  });

  it('liveTemplateOverride tracks the fired template_json', () => {
    live.set({ reference: 'A', text: 'a', template_json: JSON.stringify({ style: { verseColor: '#111' } }) });
    expect(get(liveTemplateOverride)?.style?.verseColor).toBe('#111');
    live.set({ reference: 'B', text: 'b', template_json: null });
    expect(get(liveTemplateOverride)).toBeNull();
  });
});
