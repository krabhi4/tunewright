import { describe, it, expect, beforeEach } from 'vitest';
import { get } from 'svelte/store';

// Minimal browser mocks (node env). Console + Terminal paths load no webfonts,
// so apply() never triggers a dynamic CSS import here.
const storage: Record<string, string> = {};
let throwOnGet = false;
const attrs: Record<string, string> = {};

(globalThis as unknown as { localStorage: Storage }).localStorage = {
	getItem: (k: string) => {
		if (throwOnGet) throw new Error('storage blocked');
		return k in storage ? storage[k] : null;
	},
	setItem: (k: string, v: string) => {
		storage[k] = String(v);
	},
	removeItem: (k: string) => {
		delete storage[k];
	}
} as unknown as Storage;

(globalThis as unknown as { document: Document }).document = {
	documentElement: {
		setAttribute: (k: string, v: string) => {
			attrs[k] = v;
		},
		classList: { remove: () => {} }
	}
} as unknown as Document;

(globalThis as unknown as { window: typeof globalThis }).window = globalThis;
(globalThis as unknown as { matchMedia: (q: string) => MediaQueryList }).matchMedia = () =>
	({ matches: false, addEventListener: () => {}, removeEventListener: () => {} }) as unknown as MediaQueryList;

import { setThemeFamily, setThemeMode, initTheme, themeFamily, themeMode } from './theme';

beforeEach(() => {
	for (const k in storage) delete storage[k];
	for (const k in attrs) delete attrs[k];
	throwOnGet = false;
});

describe('theme store', () => {
	it('preserves an explicit light choice across a dark-native round-trip', () => {
		setThemeFamily('console');
		setThemeMode('light');
		expect(get(themeMode)).toBe('light');
		expect(attrs['data-mode']).toBe('light');
		expect(storage['tunewright-theme-mode']).toBe('light');

		// Visit a dark-native family: display goes dark, but the stored choice stays.
		setThemeFamily('terminal');
		expect(attrs['data-theme']).toBe('terminal');
		expect(attrs['data-mode']).toBe('dark');
		expect(get(themeMode)).toBe('dark');
		expect(storage['tunewright-theme-mode']).toBe('light'); // NOT clobbered
		expect(storage['tunewright-theme-family']).toBe('terminal');

		// Return to a light-capable family: the light preference is restored.
		setThemeFamily('console');
		expect(attrs['data-mode']).toBe('light');
		expect(get(themeMode)).toBe('light');
	});

	it('ignores setThemeMode for dark-native families', () => {
		setThemeFamily('terminal');
		setThemeMode('light');
		expect(get(themeMode)).toBe('dark');
		expect(attrs['data-mode']).toBe('dark');
		expect(storage['tunewright-theme-mode']).toBeUndefined();
	});

	it('setThemeFamily persists family without writing the mode key', () => {
		setThemeFamily('terminal');
		expect(storage['tunewright-theme-family']).toBe('terminal');
		expect(storage['tunewright-theme-mode']).toBeUndefined();
		expect(attrs['data-mode']).toBe('dark');
	});

	it('does not throw when localStorage access is blocked', () => {
		throwOnGet = true;
		expect(() => initTheme()).not.toThrow();
		expect(get(themeFamily)).toBe('console');
	});
});
