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

function apply(t: StoredTheme) {
	themeFamily.set(t.family);
	themeMode.set(t.mode);
	if (typeof document === 'undefined') return;
	const root = document.documentElement;
	root.setAttribute('data-theme', t.family);
	root.setAttribute('data-mode', t.mode);
	root.classList.remove('light'); // remove the legacy hook
}

function persist(t: StoredTheme) {
	try {
		localStorage.setItem(FAMILY_KEY, t.family);
		localStorage.setItem(MODE_KEY, t.mode);
	} catch {
		/* ignore quota errors */
	}
}

export function initTheme() {
	if (typeof window === 'undefined') return;

	// one-time migration from the old single-key scheme
	const legacy = migrateLegacy(localStorage.getItem(LEGACY_KEY));
	if (legacy && !localStorage.getItem(FAMILY_KEY)) {
		persist(legacy);
		localStorage.removeItem(LEGACY_KEY);
	}

	const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
	const resolved = resolveTheme(
		localStorage.getItem(FAMILY_KEY),
		localStorage.getItem(MODE_KEY),
		prefersDark
	);
	apply(resolved);

	// follow system mode only when the user has not chosen one
	const mq = window.matchMedia('(prefers-color-scheme: dark)');
	mq.addEventListener('change', (e) => {
		if (!localStorage.getItem(MODE_KEY)) {
			let nextFamily: ThemeFamily = 'console';
			themeFamily.subscribe((f) => (nextFamily = f))();
			apply({ family: nextFamily, mode: coerceMode(nextFamily, e.matches ? 'dark' : 'light') });
		}
	});
}

export function setThemeFamily(family: ThemeFamily) {
	let mode: ThemeMode = 'dark';
	themeMode.subscribe((m) => (mode = m))();
	const next: StoredTheme = { family, mode: coerceMode(family, mode) };
	apply(next);
	persist(next);
}

export function setThemeMode(mode: ThemeMode) {
	let family: ThemeFamily = 'console';
	themeFamily.subscribe((f) => (family = f))();
	if (!familySupportsLight(family)) return; // dark-native families ignore mode changes
	const next: StoredTheme = { family, mode };
	apply(next);
	persist(next);
}

/** Back-compat: existing toolbar button calls toggleTheme(). */
export function toggleTheme() {
	let family: ThemeFamily = 'console';
	let mode: ThemeMode = 'dark';
	themeFamily.subscribe((f) => (family = f))();
	themeMode.subscribe((m) => (mode = m))();
	if (!familySupportsLight(family)) return;
	setThemeMode(mode === 'dark' ? 'light' : 'dark');
}
