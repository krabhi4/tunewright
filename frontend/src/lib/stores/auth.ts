import { writable } from 'svelte/store';

export interface AuthState {
	checked: boolean;
	setupRequired: boolean;
	authenticated: boolean;
	user: { username: string; role: 'super_admin' | 'admin' } | null;
}

export const auth = writable<AuthState>({
	checked: false,
	setupRequired: false,
	authenticated: false,
	user: null
});
