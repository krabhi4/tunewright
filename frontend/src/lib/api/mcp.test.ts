import { describe, it, expect, vi, beforeEach } from 'vitest';
import { getMcpConfig, listMcpTokens, createPat, revokeMcpToken } from './mcp';
import * as client from './client';

vi.mock('./client', () => ({
	apiFetch: vi.fn()
}));

describe('mcp api', () => {
	beforeEach(() => {
		vi.clearAllMocks();
	});

	it('getMcpConfig calls /mcp/config', async () => {
		vi.mocked(client.apiFetch).mockResolvedValueOnce({ url: 'https://music.example.com/mcp' });

		const result = await getMcpConfig();
		expect(client.apiFetch).toHaveBeenCalledWith('/mcp/config');
		expect(result.url).toBe('https://music.example.com/mcp');
	});

	it('listMcpTokens calls /mcp/tokens', async () => {
		const mockTokens = [
			{
				id: 'tok-1',
				user_id: 'u1',
				username: 'admin',
				token_type: 'pat',
				scope: 'mcp:read',
				created_at: '2026-01-01T00:00:00Z'
			}
		];
		vi.mocked(client.apiFetch).mockResolvedValueOnce(mockTokens);

		const result = await listMcpTokens();
		expect(client.apiFetch).toHaveBeenCalledWith('/mcp/tokens');
		expect(result).toEqual(mockTokens);
	});

	it('createPat sends POST request to /mcp/tokens with default scope', async () => {
		const mockResponse = {
			id: 'tok-2',
			name: 'CLI',
			token: 'tw_pat_123',
			scope: 'mcp:read mcp:write',
			created_at: '2026-01-01T00:00:00Z'
		};
		vi.mocked(client.apiFetch).mockResolvedValueOnce(mockResponse);

		const result = await createPat('CLI');
		expect(client.apiFetch).toHaveBeenCalledWith('/mcp/tokens', {
			method: 'POST',
			body: JSON.stringify({ name: 'CLI', scope: 'mcp:read mcp:write' })
		});
		expect(result).toEqual(mockResponse);
	});

	it('createPat sends custom scope if provided', async () => {
		const mockResponse = {
			id: 'tok-3',
			name: 'Read Only',
			token: 'tw_pat_456',
			scope: 'mcp:read',
			created_at: '2026-01-01T00:00:00Z'
		};
		vi.mocked(client.apiFetch).mockResolvedValueOnce(mockResponse);

		const result = await createPat('Read Only', 'mcp:read');
		expect(client.apiFetch).toHaveBeenCalledWith('/mcp/tokens', {
			method: 'POST',
			body: JSON.stringify({ name: 'Read Only', scope: 'mcp:read' })
		});
		expect(result).toEqual(mockResponse);
	});

	it('revokeMcpToken sends DELETE request to /mcp/tokens/:id', async () => {
		vi.mocked(client.apiFetch).mockResolvedValueOnce(undefined);

		await revokeMcpToken('tok/special id');
		expect(client.apiFetch).toHaveBeenCalledWith('/mcp/tokens/tok%2Fspecial%20id', {
			method: 'DELETE'
		});
	});
});
