import { goto } from '$app/navigation';

const BASE = '/api/v1';

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
		if (res.status === 401 && !path.startsWith('/auth/')) {
			goto('/login');
		}
		const body = await res.json().catch(() => ({ error: res.statusText }));
		throw new ApiError(res.status, body.error || res.statusText);
	}

	return res.json();
}
