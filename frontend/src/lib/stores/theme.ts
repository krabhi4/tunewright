import { writable } from 'svelte/store';
import {
	resolveTheme,
	migrateLegacy,
	coerceMode,
	familySupportsLight,
	type ThemeFamily,
	type ThemeMode,
	type StoredTheme
} from '$lib/theme/resolve';

export type { ThemeFamily, ThemeMode } from '$lib/theme/resolve';

const FAMILY_KEY = 'tagstudio-theme-family';
const MODE_KEY = 'tagstudio-theme-mode';
const LEGACY_KEY = 'tagstudio-theme';

export const themeFamily = writable<ThemeFamily>('console');
export const themeMode = writable<ThemeMode>('dark');

// localStorage / matchMedia can throw (Safari private mode, disabled storage,
// sandboxed iframe). Every access is guarded so theme handling can never throw
// and break the app's mount path.
function lsGet(key: string): string | null {
	try {
		return localStorage.getItem(key);
	} catch {
		return null;
	}
}
function lsSet(key: string, value: string) {
	try {
		localStorage.setItem(key, value);
	} catch {
		/* ignore quota / access errors */
	}
}
function lsRemove(key: string) {
	try {
		localStorage.removeItem(key);
	} catch {
		/* ignore */
	}
}
function prefersDark(): boolean {
	try {
		return window.matchMedia('(prefers-color-scheme: dark)').matches;
	} catch {
		return true;
	}
}

// Lazy per-theme webfont loading. Console (IBM Plex) is imported statically in
// app.css; other families pull their faces on demand the first time they activate,
// so a Console-only user never downloads Fraunces / Hanken Grotesk.
const FONT_LOADERS: Record<ThemeFamily, Array<() => Promise<unknown>>> = {
	console: [],
	editorial: [
		() => import('@fontsource/fraunces/400.css'),
		() => import('@fontsource/fraunces/600.css'),
		() => import('@fontsource/hanken-grotesk/400.css'),
		() => import('@fontsource/hanken-grotesk/500.css'),
		() => import('@fontsource/hanken-grotesk/600.css')
	],
	terminal: [],
	daw: [
		() => import('@fontsource/hanken-grotesk/400.css'),
		() => import('@fontsource/hanken-grotesk/500.css'),
		() => import('@fontsource/hanken-grotesk/600.css')
	]
};
const loadedFonts = new Set<ThemeFamily>();
function loadFonts(family: ThemeFamily) {
	if (loadedFonts.has(family)) return;
	const loaders = FONT_LOADERS[family];
	if (loaders.length === 0) return;
	loadedFonts.add(family);
	// On failure, clear the flag so a later activation of this family retries
	// instead of being stuck on fallback fonts for the rest of the session.
	Promise.all(loaders.map((load) => load())).catch(() => loadedFonts.delete(family));
}

function apply(t: StoredTheme) {
	themeFamily.set(t.family);
	themeMode.set(t.mode);
	if (typeof document === 'undefined') return;
	const root = document.documentElement;
	root.setAttribute('data-theme', t.family);
	root.setAttribute('data-mode', t.mode);
	root.classList.remove('light'); // remove the legacy hook
	loadFonts(t.family);
}

function currentFamily(): ThemeFamily {
	let family: ThemeFamily = 'console';
	themeFamily.subscribe((f) => (family = f))();
	return family;
}

let mqBound = false;

export function initTheme() {
	if (typeof window === 'undefined') return;

	// one-time migration from the old single-key scheme
	const legacy = migrateLegacy(lsGet(LEGACY_KEY));
	if (legacy && !lsGet(FAMILY_KEY)) {
		lsSet(FAMILY_KEY, legacy.family);
		lsSet(MODE_KEY, legacy.mode);
		lsRemove(LEGACY_KEY);
	}

	apply(resolveTheme(lsGet(FAMILY_KEY), lsGet(MODE_KEY), prefersDark()));

	// follow system mode only while the user has not made an explicit choice.
	// Bind once (module-level guard) so an HMR re-mount can't stack listeners.
	if (!mqBound) {
		mqBound = true;
		try {
			window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', (e) => {
				if (lsGet(MODE_KEY)) return; // user has an explicit mode; don't override
				const family = currentFamily();
				apply({ family, mode: coerceMode(family, e.matches ? 'dark' : 'light') });
			});
		} catch {
			/* matchMedia unsupported */
		}
	}
}

/**
 * Switch theme family. The user's explicit appearance (light/dark) choice is
 * preserved: the stored mode is never overwritten here, so a round-trip through
 * a dark-native family (Terminal/DAW) and back to Console/Editorial restores the
 * saved light/dark preference rather than clobbering it to dark.
 */
export function setThemeFamily(family: ThemeFamily) {
	apply(resolveTheme(family, lsGet(MODE_KEY), prefersDark()));
	lsSet(FAMILY_KEY, family);
}

/** Set light/dark appearance. No-op for dark-native families. Records the choice. */
export function setThemeMode(mode: ThemeMode) {
	const family = currentFamily();
	if (!familySupportsLight(family)) return;
	apply({ family, mode });
	lsSet(MODE_KEY, mode);
}
