import { writable, type Writable } from 'svelte/store';
import { browser } from '$app/environment';

function typesMatch(val: any, initial: any): boolean {
	if (initial === null || initial === undefined) return true;
	if (val === null || val === undefined) return false;

	if (typeof initial !== typeof val) return false;

	if (Array.isArray(initial)) {
		return Array.isArray(val);
	}
	if (typeof initial === 'object') {
		return !Array.isArray(val);
	}

	return true;
}

/**
 * A writable store that syncs with localStorage.
 * Reads initial value from localStorage on creation, writes on every update.
 */
export function persisted<T>(key: string, initial: T): Writable<T> {
	let value = initial;

	if (browser) {
		try {
			const stored = localStorage.getItem(key);
			if (stored !== null) {
				const parsed = JSON.parse(stored);
				if (typesMatch(parsed, initial)) {
					value = parsed;
				}
			}
		} catch {
			// ignore parse errors
		}
	}

	const store = writable<T>(value);

	if (browser) {
		store.subscribe((val) => {
			try {
				localStorage.setItem(key, JSON.stringify(val));
			} catch {
				// ignore quota errors
			}
		});
	}

	return store;
}
