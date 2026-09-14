// Self-hosted fonts (offline-first — bundled by Vite, never fetched at runtime).
// Shared by every entry point (console + output windows). Weights match the
// design system in app.css / docs/design.
import '@fontsource/space-grotesk/500.css';
import '@fontsource/space-grotesk/600.css';
import '@fontsource/space-grotesk/700.css';
import '@fontsource/inter/400.css';
import '@fontsource/inter/500.css';
import '@fontsource/inter/600.css';
import '@fontsource/inter/700.css';
import '@fontsource/fraunces/400.css';
import '@fontsource/fraunces/600.css';
import '@fontsource/fraunces/400-italic.css';
import '@fontsource/playfair-display/600.css';
import '@fontsource/playfair-display/700.css';
import '@fontsource/playfair-display/400-italic.css';
// IBM Plex Mono is the console's figure face: every clock, confidence, latency
// and verse number renders in it, so a changing number never reflows the row
// beside it (docs/REBRAND.md §1). JetBrains Mono stays because TEMPLATES still
// offer it by name — that is a choice an operator saved, not chrome.
import '@fontsource/ibm-plex-mono/400.css';
import '@fontsource/ibm-plex-mono/500.css';
import '@fontsource/ibm-plex-mono/600.css';
import '@fontsource/jetbrains-mono/400.css';
import '@fontsource/jetbrains-mono/500.css';
