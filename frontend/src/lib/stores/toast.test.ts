import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { get } from 'svelte/store';
import { toasts, toast, dismissToast } from './toast';

describe('toast store', () => {
	beforeEach(() => {
		vi.useFakeTimers();
		toasts.set([]);
	});

	afterEach(() => {
		vi.useRealTimers();
	});

	it('adds toasts of each kind with unique ids', () => {
		toast.success('saved');
		toast.error('failed');
		toast.warning('careful');
		toast.info('fyi');

		const list = get(toasts);
		expect(list).toHaveLength(4);
		expect(list[0]).toMatchObject({ kind: 'success', message: 'saved' });
		expect(list[1]).toMatchObject({ kind: 'error', message: 'failed' });
		expect(list[2]).toMatchObject({ kind: 'warning', message: 'careful' });
		expect(list[3]).toMatchObject({ kind: 'info', message: 'fyi' });
		expect(new Set(list.map((t) => t.id)).size).toBe(4);
	});

	it('auto-dismisses after its duration', () => {
		toast.info('hello');
		expect(get(toasts)).toHaveLength(1);
		vi.advanceTimersByTime(4000);
		expect(get(toasts)).toHaveLength(0);
	});

	it('errors are sticky by default until manually dismissed', () => {
		const id = toast.error('bad');
		vi.advanceTimersByTime(60000);
		expect(get(toasts)).toHaveLength(1);
		dismissToast(id);
		expect(get(toasts)).toHaveLength(0);
	});

	it('warnings auto-dismiss after 8s', () => {
		toast.warning('careful');
		vi.advanceTimersByTime(7999);
		expect(get(toasts)).toHaveLength(1);
		vi.advanceTimersByTime(1);
		expect(get(toasts)).toHaveLength(0);
	});

	it('dismissToast removes only the targeted toast', () => {
		toast.info('a');
		toast.info('b');
		const [a] = get(toasts);
		dismissToast(a.id);
		const rest = get(toasts);
		expect(rest).toHaveLength(1);
		expect(rest[0].message).toBe('b');
	});

	it('caps visible toasts, dropping the oldest first', () => {
		for (let i = 0; i < 7; i++) toast.info(`t${i}`);
		const list = get(toasts);
		expect(list).toHaveLength(5);
		expect(list[0].message).toBe('t2');
		expect(list[4].message).toBe('t6');
	});

	it('supports a custom duration', () => {
		toast.info('quick', { duration: 1000 });
		vi.advanceTimersByTime(999);
		expect(get(toasts)).toHaveLength(1);
		vi.advanceTimersByTime(1);
		expect(get(toasts)).toHaveLength(0);
	});

	it('duration 0 makes a toast sticky until manually dismissed', () => {
		const id = toast.error('sticky', { duration: 0 });
		vi.advanceTimersByTime(60000);
		expect(get(toasts)).toHaveLength(1);
		dismissToast(id);
		expect(get(toasts)).toHaveLength(0);
	});
});
