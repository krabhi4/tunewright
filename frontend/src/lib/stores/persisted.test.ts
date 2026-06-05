import { describe, it, expect, beforeEach, vi } from 'vitest';
import { get } from 'svelte/store';

vi.mock('$app/environment', () => ({
	browser: true
}));

const storage: Record<string, string> = {};

(globalThis as unknown as { localStorage: Storage }).localStorage = {
	getItem: (k: string) => {
		return k in storage ? storage[k] : null;
	},
	setItem: (k: string, v: string) => {
		storage[k] = String(v);
	},
	removeItem: (k: string) => {
		delete storage[k];
	}
} as unknown as Storage;

import { persisted } from './persisted';

beforeEach(() => {
	for (const k in storage) delete storage[k];
});

describe('persisted store', () => {
	it('uses initial value when localStorage is empty', () => {
		const store = persisted('test-key', 123);
		expect(get(store)).toBe(123);
	});

	it('loads valid value of same type from localStorage', () => {
		storage['test-key'] = '456';
		const store = persisted('test-key', 123);
		expect(get(store)).toBe(456);
	});

	it('rejects wrong-typed value and falls back to initial value', () => {
		// Wrong type: string instead of number
		storage['test-key'] = '"wrong-string"';
		const store = persisted('test-key', 123);
		expect(get(store)).toBe(123);
	});

	it('rejects array when object is expected', () => {
		storage['test-key'] = '[1, 2, 3]';
		const store = persisted('test-key', { a: 1 });
		expect(get(store)).toEqual({ a: 1 });
	});

	it('rejects object when array is expected', () => {
		storage['test-key'] = '{"a": 1}';
		const store = persisted('test-key', [1, 2]);
		expect(get(store)).toEqual([1, 2]);
	});

	it('accepts array when array is expected', () => {
		storage['test-key'] = '[1, 2]';
		const store = persisted('test-key', [3, 4]);
		expect(get(store)).toEqual([1, 2]);
	});
});
