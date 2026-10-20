import { apiFetch } from './client';

export interface McpToken {
	id: string;
	user_id: string;
	username: string;
	client_id?: string;
	client_name?: string;
	name?: string;
	token_type: string;
	scope: string;
	created_at: string;
	expires_at?: string;
	last_used_at?: string;
}

export interface CreatedPatResponse {
	id: string;
	name: string;
	token: string;
	scope: string;
	created_at: string;
}

export async function getMcpConfig(): Promise<{ url: string }> {
	return apiFetch<{ url: string }>('/mcp/config');
}

export async function listMcpTokens(): Promise<McpToken[]> {
	return apiFetch<McpToken[]>('/mcp/tokens');
}

export async function createPat(name: string, scope = 'mcp:read mcp:write'): Promise<CreatedPatResponse> {
	return apiFetch<CreatedPatResponse>('/mcp/tokens', {
		method: 'POST',
		body: JSON.stringify({ name, scope })
	});
}

export async function revokeMcpToken(id: string): Promise<void> {
	return apiFetch<void>(`/mcp/tokens/${encodeURIComponent(id)}`, {
		method: 'DELETE'
	});
}
