import { describe, it, expect } from 'vitest';
import {
	THEME_FAMILIES,
	familySupportsLight,
	resolveTheme,
	migrateLegacy,
	type StoredTheme
} from './resolve';

describe('familySupportsLight', () => {
	it('allows light for console and editorial', () => {
		expect(familySupportsLight('console')).toBe(true);
		expect(familySupportsLight('editorial')).toBe(true);
	});
	it('forces dark for terminal and daw', () => {
		expect(familySupportsLight('terminal')).toBe(false);
		expect(familySupportsLight('daw')).toBe(false);
	});
});

describe('resolveTheme', () => {
	it('defaults to console + system mode when nothing stored', () => {
		expect(resolveTheme(null, null, true)).toEqual({ family: 'console', mode: 'dark' });
		expect(resolveTheme(null, null, false)).toEqual({ family: 'console', mode: 'light' });
	});
	it('uses stored family and mode when valid', () => {
		expect(resolveTheme('editorial', 'light', true)).toEqual({ family: 'editorial', mode: 'light' });
	});
	it('coerces mode to dark for dark-native families', () => {
		expect(resolveTheme('terminal', 'light', false)).toEqual({ family: 'terminal', mode: 'dark' });
	});
	it('ignores invalid stored values', () => {
		expect(resolveTheme('bogus', 'bogus', true)).toEqual({ family: 'console', mode: 'dark' });
	});
});

describe('migrateLegacy', () => {
	it('maps a legacy "light" theme value to console light', () => {
		expect(migrateLegacy('light')).toEqual({ family: 'console', mode: 'light' });
	});
	it('maps a legacy "dark" theme value to console dark', () => {
		expect(migrateLegacy('dark')).toEqual({ family: 'console', mode: 'dark' });
	});
	it('returns null for anything else', () => {
		expect(migrateLegacy(null)).toBeNull();
		expect(migrateLegacy('whatever')).toBeNull();
	});
});

describe('THEME_FAMILIES', () => {
	it('lists all four families', () => {
		expect(THEME_FAMILIES).toEqual(['console', 'editorial', 'terminal', 'daw']);
	});
});
