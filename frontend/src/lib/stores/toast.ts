import { writable } from 'svelte/store';

export type ToastKind = 'success' | 'error' | 'warning' | 'info';

export interface Toast {
	id: number;
	kind: ToastKind;
	message: string;
}

interface ToastOptions {
	/** ms until auto-dismiss; 0 = sticky until manually dismissed */
	duration?: number;
}

/** Most toasts shown at once; the oldest is dropped beyond this. */
const MAX_TOASTS = 5;

/**
 * Errors are sticky (manual dismiss) so a failure can't scroll away unseen,
 * preserving the guaranteed-acknowledgement property of the alert() calls
 * toasts replaced. Successes clear quickly.
 */
const DEFAULT_DURATION: Record<ToastKind, number> = {
	success: 4000,
	info: 4000,
	warning: 8000,
	error: 0
};

export const toasts = writable<Toast[]>([]);

let nextId = 0;
const timers = new Map<number, ReturnType<typeof setTimeout>>();

function clearTimer(id: number) {
	const timer = timers.get(id);
	if (timer) {
		clearTimeout(timer);
		timers.delete(id);
	}
}

export function dismissToast(id: number) {
	clearTimer(id);
	toasts.update((list) => list.filter((t) => t.id !== id));
}

function push(kind: ToastKind, message: string, opts?: ToastOptions): number {
	const id = ++nextId;
	const duration = opts?.duration ?? DEFAULT_DURATION[kind];

	toasts.update((list) => {
		const next = [...list, { id, kind, message }];
		while (next.length > MAX_TOASTS) {
			const dropped = next.shift();
			if (dropped) clearTimer(dropped.id);
		}
		return next;
	});

	if (duration > 0) {
		timers.set(
			id,
			setTimeout(() => dismissToast(id), duration)
		);
	}
	return id;
}

export const toast = {
	success: (message: string, opts?: ToastOptions) => push('success', message, opts),
	error: (message: string, opts?: ToastOptions) => push('error', message, opts),
	warning: (message: string, opts?: ToastOptions) => push('warning', message, opts),
	info: (message: string, opts?: ToastOptions) => push('info', message, opts)
};
