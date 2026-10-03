import { goto } from '$app/navigation';
import { auth } from '$lib/stores/auth';
import { clearTags } from '$lib/stores/tags';

const BASE = '/api/v1';
const PUBLIC_AUTH_PATHS = ['/auth/login', '/auth/setup', '/auth/register', '/auth/logout', '/auth/check'];

export class ApiError extends Error {
	constructor(
		public status: number,
		message: string
	) {
		super(message);
	}
}

export async function apiFetch<T>(path: string, init?: RequestInit): Promise<T> {
	// FormData bodies set their own multipart Content-Type (with boundary),
	// so only force JSON for everything else.
	const isFormData = init?.body instanceof FormData;
	const res = await fetch(`${BASE}${path}`, {
		...init,
		headers: {
			...(isFormData ? {} : { 'Content-Type': 'application/json' }),
			...init?.headers
		}
	});

	if (!res.ok) {
		if (res.status === 401 && !PUBLIC_AUTH_PATHS.includes(path)) {
			auth.update((s) => ({ ...s, authenticated: false, user: null }));
			clearTags(true);
			goto('/login');
		}
		const body = await res.json().catch(() => ({ error: res.statusText }));
		throw new ApiError(res.status, body.error || res.statusText);
	}

	return res.json();
}
