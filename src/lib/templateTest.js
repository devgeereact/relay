// ONE source of truth for previewing a template and pushing it to the real
// screens, shared by the Templates gallery and the editor so the two surfaces
// can never drift (Decision §26).
//
// "Test on screens" is NOT a new backend path — it is a normal manual fire of a
// sample verse with the template as the content-type override, exactly the fire
// an operator clears with Esc. The verse is real scripture (John 3:16) so the
// template renders the way it will in a service, not against lorem text.
import { manualFire } from './stores/capture.js';

/** The stand-in content every template preview and test uses. Matches the
 *  gallery thumbnails' sample so a preview looks like its card. */
export const SAMPLE_TEST_CONTENT = {
  reference: 'John 3:16',
  text: 'For God so loved the world, that he gave his only begotten Son, that whosoever believeth in him should not perish, but have everlasting life.',
  translation: 'KJV',
};

/**
 * Push the sample verse to the live screens using `templateId`'s look. It is a
 * real fire (it appears on every open output and persists like any manual fire),
 * so the operator clears it with Esc. Throws on backend failure — the caller
 * surfaces it through the ONE humaniser.
 */
export async function testTemplateOnOutputs(templateId) {
  await manualFire(SAMPLE_TEST_CONTENT.reference, 'Template test — press Esc to clear', templateId);
}
