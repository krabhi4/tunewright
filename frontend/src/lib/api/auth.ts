import { apiFetch } from './client';

export interface AuthCheck {
	setup_required?: boolean;
	authenticated?: boolean;
	user?: { username: string; role: 'super_admin' | 'admin' };
}

export interface UserInfo {
	id: string;
	username: string;
	role: 'super_admin' | 'admin';
	created_at: string;
}

export interface InviteInfo {
	token: string;
	created_by: string;
	expires_at: string;
	link: string;
}

export async function checkAuth(): Promise<AuthCheck> {
	const res = await fetch('/api/v1/auth/check');
	return res.json();
}

export interface AuthResponse {
	status: string;
	user?: { username: string; role: 'super_admin' | 'admin' };
}

export function login(username: string, password: string) {
	return apiFetch<AuthResponse>('/auth/login', {
		method: 'POST',
		body: JSON.stringify({ username, password })
	});
}

export function logout() {
	return apiFetch<{ status: string }>('/auth/logout', { method: 'POST' });
}

export function setup(username: string, password: string) {
	return apiFetch<AuthResponse>('/auth/setup', {
		method: 'POST',
		body: JSON.stringify({ username, password })
	});
}

export function register(token: string, username: string, password: string) {
	return apiFetch<AuthResponse>('/auth/register', {
		method: 'POST',
		body: JSON.stringify({ token, username, password })
	});
}

export function listUsers() {
	return apiFetch<UserInfo[]>('/auth/users');
}

export function deleteUser(id: string) {
	return apiFetch<{ status: string }>(`/auth/users/${id}`, { method: 'DELETE' });
}

export function createInvite() {
	return apiFetch<InviteInfo>('/auth/invites', { method: 'POST' });
}

export function listInvites() {
	return apiFetch<InviteInfo[]>('/auth/invites');
}

export function deleteInvite(token: string) {
	return apiFetch<{ status: string }>(`/auth/invites/${token}`, { method: 'DELETE' });
}
