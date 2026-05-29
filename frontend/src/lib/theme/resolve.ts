export type ThemeFamily = 'console' | 'editorial' | 'terminal' | 'daw';
export type ThemeMode = 'dark' | 'light';

export const THEME_FAMILIES: ThemeFamily[] = ['console', 'editorial', 'terminal', 'daw'];

const DARK_NATIVE: ReadonlySet<ThemeFamily> = new Set(['terminal', 'daw']);

export interface StoredTheme {
	family: ThemeFamily;
	mode: ThemeMode;
}

function isFamily(v: unknown): v is ThemeFamily {
	return typeof v === 'string' && (THEME_FAMILIES as string[]).includes(v);
}

function isMode(v: unknown): v is ThemeMode {
	return v === 'dark' || v === 'light';
}

export function familySupportsLight(family: ThemeFamily): boolean {
	return !DARK_NATIVE.has(family);
}

/** Force-correct a mode against a family's capabilities. */
export function coerceMode(family: ThemeFamily, mode: ThemeMode): ThemeMode {
	return familySupportsLight(family) ? mode : 'dark';
}

/**
 * Decide the active theme from stored values + system preference.
 * Invalid stored values fall back to the console default.
 */
export function resolveTheme(
	storedFamily: string | null,
	storedMode: string | null,
	prefersDark: boolean
): StoredTheme {
	const family: ThemeFamily = isFamily(storedFamily) ? storedFamily : 'console';
	const mode: ThemeMode = isMode(storedMode) ? storedMode : prefersDark ? 'dark' : 'light';
	return { family, mode: coerceMode(family, mode) };
}

/** Translate the old single `tagstudio-theme` value into the new shape. */
export function migrateLegacy(legacy: string | null): StoredTheme | null {
	if (legacy === 'light') return { family: 'console', mode: 'light' };
	if (legacy === 'dark') return { family: 'console', mode: 'dark' };
	return null;
}
