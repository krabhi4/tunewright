import { writable } from 'svelte/store';

export interface AuthState {
	checked: boolean;
	setupRequired: boolean;
	/** Whether /auth/setup requires a TUNEWRIGHT_SETUP_TOKEN value. */
	setupTokenRequired?: boolean;
	authenticated: boolean;
	user: { username: string; role: 'super_admin' | 'admin' } | null;
}

export const auth = writable<AuthState>({
	checked: false,
	setupRequired: false,
	setupTokenRequired: false,
	authenticated: false,
	user: null
});
