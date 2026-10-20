<script lang="ts">
	import Modal from '$lib/components/common/Modal.svelte';
	import {
		getMcpConfig,
		listMcpTokens,
		createPat,
		revokeMcpToken,
		type McpToken,
		type CreatedPatResponse
	} from '$lib/api/mcp';
	import { auth } from '$lib/stores/auth';
	import { toast } from '$lib/stores/toast';
	import { copyText } from '$lib/utils/clipboard';

	interface Props {
		open: boolean;
		onClose: () => void;
	}

	let { open, onClose }: Props = $props();

	type MainTab = 'setup' | 'pat' | 'connections';
	type ClientTab = 'claudecode' | 'cursor' | 'vscode' | 'desktop' | 'claudeai';

	const CLIENT_TABS: { id: ClientTab; label: string }[] = [
		{ id: 'claudecode', label: 'Claude Code' },
		{ id: 'cursor', label: 'Cursor' },
		{ id: 'vscode', label: 'VS Code' },
		{ id: 'desktop', label: 'Claude Desktop' },
		{ id: 'claudeai', label: 'Claude.ai' }
	];

	let activeMainTab = $state<MainTab>('setup');
	let activeClientTab = $state<ClientTab>('claudecode');

	let origin = $state('http://127.0.0.1:8080');
	let serverMcpUrl = $state<string | null>(null);

	let tokens = $state<McpToken[]>([]);
	let loadingTokens = $state(false);
	let error = $state('');

	let patName = $state('');
	let patScope = $state('mcp:read mcp:write');
	let creatingPat = $state(false);
	let newlyCreatedPat = $state<CreatedPatResponse | null>(null);

	let revokingId = $state<string | null>(null);

	$effect(() => {
		if (typeof window !== 'undefined') {
			origin = window.location.origin;
		}
	});

	$effect(() => {
		if (open) {
			loadTokens();
			getMcpConfig()
				.then((config) => (serverMcpUrl = config.url))
				.catch(() => (serverMcpUrl = null));
		} else {
			newlyCreatedPat = null;
			patName = '';
			error = '';
		}
	});

	async function loadTokens() {
		loadingTokens = true;
		error = '';
		try {
			tokens = await listMcpTokens();
		} catch (err: any) {
			error = err.message || 'Failed to load connections';
			toast.error(error);
		} finally {
			loadingTokens = false;
		}
	}

	async function handleCopy(text: string, label = 'Copied to clipboard') {
		const ok = await copyText(text);
		if (ok) {
			toast.success(label);
		} else {
			toast.error('Failed to copy');
		}
	}

	async function handleCreatePat(e: SubmitEvent) {
		e.preventDefault();
		if (!patName.trim() || creatingPat) return;
		creatingPat = true;
		error = '';
		try {
			const res = await createPat(patName.trim(), patScope);
			newlyCreatedPat = res;
			patName = '';
			toast.success('Personal Access Token created');
			await loadTokens();
		} catch (err: any) {
			error = err.message || 'Failed to create token';
			toast.error(error);
		} finally {
			creatingPat = false;
		}
	}

	async function handleRevokeToken(token: McpToken) {
		if (revokingId) return;
		revokingId = token.id;
		try {
			await revokeMcpToken(token.id);
			toast.success(`Revoked ${token.client_name || token.name || 'token'}`);
		} catch (err: any) {
			toast.error(err.message || 'Failed to revoke');
		} finally {
			revokingId = null;
			await loadTokens();
		}
	}

	function formatDate(iso: string): string {
		try {
			const d = new Date(iso);
			if (isNaN(d.getTime())) return iso;
			return d.toLocaleDateString(undefined, {
				month: 'short',
				day: 'numeric',
				year: 'numeric',
				hour: '2-digit',
				minute: '2-digit'
			});
		} catch {
			return iso;
		}
	}

	function json(value: unknown): string {
		return JSON.stringify(value, null, 2);
	}

	let patList = $derived(
		tokens.filter((t) => t.token_type === 'pat' && t.username === $auth.user?.username)
	);
	let mcpUrl = $derived(serverMcpUrl ?? `${origin}/mcp`);
	let mcpOrigin = $derived(new URL(mcpUrl).origin);
	let patValue = $derived(newlyCreatedPat?.token || '<YOUR_PAT>');
	let isPublicHttps = $derived(
		mcpOrigin.startsWith('https://') &&
			!/^https:\/\/(localhost|127\.|10\.|192\.168\.|172\.(1[6-9]|2\d|3[01])\.|\[)/.test(mcpOrigin)
	);

	let claudeAddCommand = $derived(`claude mcp add --transport http --scope user tunewright ${mcpUrl}`);
	let claudePatCommand = $derived(
		`claude mcp add --transport http --scope user tunewright ${mcpUrl} --header "Authorization: Bearer ${patValue}"`
	);
	let cursorSnippet = $derived(json({ mcpServers: { tunewright: { url: mcpUrl } } }));
	let cursorPatSnippet = $derived(
		json({
			mcpServers: { tunewright: { url: mcpUrl, headers: { Authorization: `Bearer ${patValue}` } } }
		})
	);
	let vscodeSnippet = $derived(json({ servers: { tunewright: { type: 'http', url: mcpUrl } } }));
	let clineSnippet = $derived(
		json({ mcpServers: { tunewright: { type: 'streamableHttp', url: mcpUrl } } })
	);
	let desktopSnippet = $derived(
		json({
			mcpServers: {
				tunewright: {
					command: 'npx',
					args: ['-y', 'mcp-remote', mcpUrl, ...(mcpUrl.startsWith('http:') ? ['--allow-http'] : [])]
				}
			}
		})
	);
</script>

{#snippet codeBlock(code: string, label = 'Copy')}
	<div class="code-box code-box-block">
		<pre>{code}</pre>
		<button class="copy-btn" onclick={() => handleCopy(code)}>{label}</button>
	</div>
{/snippet}

{#snippet codeLine(code: string)}
	<div class="code-box">
		<code>{code}</code>
		<button class="copy-btn" onclick={() => handleCopy(code)}>Copy</button>
	</div>
{/snippet}

{#snippet patLink()}
	<button class="link-btn" onclick={() => (activeMainTab = 'pat')}>Personal Access Tokens tab</button>
{/snippet}

<Modal title="Connect Model Context Protocol (MCP)" {open} {onClose} wide={true}>
	<div class="mcp-modal">
		<div class="mcp-main-tabs" role="tablist" aria-label="MCP sections">
			<button
				class="mcp-main-tab"
				class:active={activeMainTab === 'setup'}
				onclick={() => (activeMainTab = 'setup')}
				role="tab"
				aria-selected={activeMainTab === 'setup'}
			>
				Client Setup
			</button>
			<button
				class="mcp-main-tab"
				class:active={activeMainTab === 'pat'}
				onclick={() => (activeMainTab = 'pat')}
				role="tab"
				aria-selected={activeMainTab === 'pat'}
			>
				Personal Access Tokens
			</button>
			<button
				class="mcp-main-tab"
				class:active={activeMainTab === 'connections'}
				onclick={() => (activeMainTab = 'connections')}
				role="tab"
				aria-selected={activeMainTab === 'connections'}
			>
				Active Connections ({tokens.length})
			</button>
		</div>

		{#if activeMainTab === 'setup'}
			<div class="mcp-client-section" role="tabpanel">
				<div class="mcp-sub-tabs" role="tablist" aria-label="MCP clients">
					{#each CLIENT_TABS as tab (tab.id)}
						<button
							class="mcp-sub-tab"
							class:active={activeClientTab === tab.id}
							onclick={() => (activeClientTab = tab.id)}
							role="tab"
							aria-selected={activeClientTab === tab.id}
						>
							{tab.label}
						</button>
					{/each}
				</div>

				<div class="mcp-content" role="tabpanel">
					{#if activeClientTab === 'claudecode'}
						<div class="client-guide">
							<h3 class="guide-title">Claude Code (CLI)</h3>
							<p class="guide-desc">
								Claude Code connects over Streamable HTTP and signs in through your browser with OAuth.
							</p>

							<div class="step-card">
								<div class="step-num">1</div>
								<div class="step-body">
									<div class="step-label">Add the Tunewright MCP server:</div>
									{@render codeLine(claudeAddCommand)}
								</div>
							</div>

							<div class="step-card">
								<div class="step-num">2</div>
								<div class="step-body">
									<div class="step-label">Sign in and approve access in the browser:</div>
									{@render codeLine('claude mcp login tunewright')}
									<p class="step-tip">
										Or run <code>/mcp</code> inside a Claude Code session. Over SSH or without a browser, run
										<code>claude mcp login tunewright --no-browser</code> and paste the redirect URL back.
									</p>
								</div>
							</div>

							<p class="step-tip">
								To skip OAuth, connect with a token from the {@render patLink()} instead:
							</p>
							{@render codeLine(claudePatCommand)}
						</div>
					{:else if activeClientTab === 'cursor'}
						<div class="client-guide">
							<h3 class="guide-title">Cursor</h3>
							<p class="guide-desc">
								Add this to <code>~/.cursor/mcp.json</code> or <code>.cursor/mcp.json</code>, then choose
								<strong>Connect</strong> next to Tunewright in Cursor's MCP settings to sign in.
							</p>
							{@render codeBlock(cursorSnippet)}
							<p class="step-tip">To use a token from the {@render patLink()} instead of OAuth:</p>
							{@render codeBlock(cursorPatSnippet)}
						</div>
					{:else if activeClientTab === 'vscode'}
						<div class="client-guide">
							<h3 class="guide-title">VS Code</h3>
							<p class="guide-desc">
								Add this to <code>.vscode/mcp.json</code>. VS Code (GitHub Copilot agent mode) asks you to sign in
								the first time it starts the server.
							</p>
							{@render codeBlock(vscodeSnippet)}
							<p class="step-tip">
								Cline: add this to <code>cline_mcp_settings.json</code> and use its Authenticate button. Roo Code uses
								<code>"type": "streamable-http"</code> with an <code>Authorization: Bearer</code> header from the
								{@render patLink()}.
							</p>
							{@render codeBlock(clineSnippet)}
						</div>
					{:else if activeClientTab === 'desktop'}
						<div class="client-guide">
							<h3 class="guide-title">Claude Desktop</h3>
							{#if isPublicHttps}
								<p class="guide-desc">
									Open <strong>Customize &rarr; Connectors</strong>, choose <strong>Add custom connector</strong> and
									enter <code>{mcpUrl}</code>. Claude signs in with OAuth.
								</p>
								<p class="step-tip">Or bridge it locally with <code>mcp-remote</code> in <code>claude_desktop_config.json</code>:</p>
							{:else}
								<p class="guide-desc">
									Custom connectors need a public HTTPS address, so bridge this server locally with
									<code>mcp-remote</code>. Add this to <code>claude_desktop_config.json</code> and restart Claude Desktop; a
									browser window opens to sign in.
								</p>
							{/if}
							{@render codeBlock(desktopSnippet)}
							<p class="step-tip">Requires Node.js (<code>npx</code>) on this machine.</p>
						</div>
					{:else if activeClientTab === 'claudeai'}
						<div class="client-guide">
							<h3 class="guide-title">Claude.ai</h3>
							<div class="step-card">
								<div class="step-body">
									<ol class="ordered-steps">
										<li>Open <strong>Claude.ai</strong> and go to <strong>Customize &rarr; Connectors</strong>.</li>
										<li>Choose <strong>Add custom connector</strong>.</li>
										<li>Enter the URL <code>{mcpUrl}</code>.</li>
										<li>Choose <strong>Connect</strong>, then sign in and approve access to Tunewright.</li>
									</ol>
									<p class="step-tip">
										Claude.ai connects from Anthropic's servers, so Tunewright must be reachable at a public HTTPS
										address, for example through a reverse proxy or a Cloudflare Tunnel.
										{#if !isPublicHttps}
											<strong>This server's address is {mcpOrigin}, which Claude.ai cannot reach.</strong>
										{/if}
									</p>
								</div>
							</div>
						</div>
					{/if}
				</div>
			</div>
		{:else if activeMainTab === 'pat'}
			<div class="pat-section" role="tabpanel">
				<h3 class="section-title">Create Personal Access Token</h3>
				<p class="section-desc">
					Personal Access Tokens authenticate clients that cannot sign in through a browser. Send one as
					<code>Authorization: Bearer &lt;token&gt;</code>. They never expire, so revoke them when no longer needed.
				</p>

				{#if newlyCreatedPat}
					<div class="token-alert" role="status">
						<div class="token-alert-header">
							<span class="token-alert-title">Token Created Successfully</span>
							<span class="token-alert-warn">Make sure to copy your token now. You will not be able to see it again!</span>
						</div>
						<div class="code-box">
							<code class="raw-token">{newlyCreatedPat.token}</code>
							<button class="copy-btn" onclick={() => handleCopy(newlyCreatedPat!.token, 'Token copied to clipboard')}>
								Copy Token
							</button>
						</div>
					</div>
				{/if}

				<form class="pat-form" onsubmit={handleCreatePat}>
					<div class="form-row">
						<div class="form-field flex-2">
							<label for="pat-name">Token Name / Client Description</label>
							<input
								id="pat-name"
								type="text"
								bind:value={patName}
								placeholder="e.g. Cursor MacBook, Dev-Laptop, CI Server"
								maxlength="100"
								required
								disabled={creatingPat}
							/>
						</div>
						<div class="form-field flex-1">
							<label for="pat-scope">Scope</label>
							<select id="pat-scope" bind:value={patScope} disabled={creatingPat}>
								<option value="mcp:read mcp:write">Full Access (Read & Write)</option>
								<option value="mcp:read">Read-Only</option>
							</select>
						</div>
						<button type="submit" class="primary-btn form-submit" disabled={creatingPat || !patName.trim()}>
							{creatingPat ? 'Generating...' : 'Create Token'}
						</button>
					</div>
				</form>

				<h4 class="sub-title">Active Personal Access Tokens</h4>
				{#if patList.length === 0}
					<div class="empty-state">No Personal Access Tokens created yet.</div>
				{:else}
					<div class="tokens-table-container">
						<table class="tokens-table">
							<thead>
								<tr>
									<th>Description</th>
									<th>Scope</th>
									<th>Created</th>
									<th>Last Used</th>
									<th>Action</th>
								</tr>
							</thead>
							<tbody>
								{#each patList as token (token.id)}
									<tr>
										<td class="cell-name">{token.name || token.id}</td>
										<td><span class="scope-badge">{token.scope}</span></td>
										<td>{formatDate(token.created_at)}</td>
										<td>{token.last_used_at ? formatDate(token.last_used_at) : 'Never'}</td>
										<td>
											<button
												class="revoke-btn"
												disabled={revokingId === token.id}
												onclick={() => handleRevokeToken(token)}
											>
												{revokingId === token.id ? 'Revoking...' : 'Revoke'}
											</button>
										</td>
									</tr>
								{/each}
							</tbody>
						</table>
					</div>
				{/if}
			</div>
		{:else if activeMainTab === 'connections'}
			<div class="connections-section" role="tabpanel">
				<div class="section-header-flex">
					<div>
						<h3 class="section-title">Authorized Clients & Sessions</h3>
						<p class="section-desc">Manage connected AI assistants, CLI sessions, and Personal Access Tokens.</p>
					</div>
					<button class="refresh-btn" onclick={loadTokens} disabled={loadingTokens}>
						{loadingTokens ? 'Refreshing...' : 'Refresh'}
					</button>
				</div>

				{#if tokens.length === 0}
					<div class="empty-state">No active MCP clients or tokens found.</div>
				{:else}
					<div class="tokens-table-container">
						<table class="tokens-table">
							<thead>
								<tr>
									<th>Application / Token</th>
									<th>Type</th>
									<th>User</th>
									<th>Scope</th>
									<th>Authorized At</th>
									<th>Last Used</th>
									<th>Action</th>
								</tr>
							</thead>
							<tbody>
								{#each tokens as token (token.id)}
									<tr>
										<td class="cell-name">
											<strong>{token.client_name || token.name || 'Unnamed Client'}</strong>
										</td>
										<td>
											<span class="type-badge" class:type-pat={token.token_type === 'pat'}>
												{token.token_type === 'pat' ? 'Access Token' : 'OAuth 2.1'}
											</span>
										</td>
										<td>{token.username}</td>
										<td><span class="scope-badge">{token.scope}</span></td>
										<td>{formatDate(token.created_at)}</td>
										<td>{token.last_used_at ? formatDate(token.last_used_at) : 'Never'}</td>
										<td>
											<button
												class="revoke-btn"
												disabled={revokingId === token.id}
												onclick={() => handleRevokeToken(token)}
											>
												{revokingId === token.id ? 'Revoking...' : 'Revoke'}
											</button>
										</td>
									</tr>
								{/each}
							</tbody>
						</table>
					</div>
				{/if}
			</div>
		{/if}
	</div>
</Modal>

<style>
	.mcp-modal {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.mcp-main-tabs {
		display: flex;
		gap: 4px;
		border-bottom: 1px solid var(--border);
		padding-bottom: 8px;
	}

	.mcp-main-tab {
		background: transparent;
		border: none;
		border-radius: var(--radius-sm);
		padding: 6px 12px;
		font-family: var(--font-ui);
		font-size: 13px;
		font-weight: 500;
		color: var(--text-secondary);
		cursor: pointer;
		transition: background 0.15s, color 0.15s;
	}

	.mcp-main-tab:hover {
		background: var(--bg-hover);
		color: var(--text-primary);
	}

	.mcp-main-tab.active {
		background: var(--accent-muted);
		color: var(--accent);
		font-weight: 600;
	}

	.mcp-client-section {
		display: flex;
		flex-direction: column;
		gap: 14px;
	}

	.mcp-sub-tabs {
		display: flex;
		gap: 6px;
		flex-wrap: wrap;
		padding: 4px;
		background: var(--bg-surface);
		border-radius: var(--radius-sm);
		border: 1px solid var(--border);
	}

	.mcp-sub-tab {
		background: transparent;
		border: none;
		border-radius: var(--radius-sm);
		padding: 5px 10px;
		font-family: var(--font-ui);
		font-size: 12px;
		color: var(--text-secondary);
		cursor: pointer;
		transition: background 0.12s, color 0.12s;
	}

	.mcp-sub-tab:hover {
		color: var(--text-primary);
	}

	.mcp-sub-tab.active {
		background: var(--bg-elevated);
		color: var(--text-primary);
		font-weight: 600;
		box-shadow: 0 1px 3px rgba(0, 0, 0, 0.2);
	}

	.client-guide {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.guide-title {
		font-size: 14px;
		font-weight: 600;
		color: var(--text-primary);
		margin: 0;
	}

	.guide-desc {
		font-size: 12px;
		color: var(--text-secondary);
		margin: 0;
		line-height: 1.4;
	}

	.step-card {
		display: flex;
		gap: 12px;
		padding: 12px;
		background: var(--bg-surface);
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
	}

	.step-num {
		width: 22px;
		height: 22px;
		border-radius: 50%;
		background: var(--accent-muted);
		color: var(--accent);
		font-size: 12px;
		font-weight: 700;
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
	}

	.step-body {
		flex: 1;
		display: flex;
		flex-direction: column;
		gap: 8px;
		min-width: 0;
	}

	.step-label {
		font-size: 12px;
		font-weight: 500;
		color: var(--text-primary);
	}

	.code-box {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 8px;
		background: var(--bg-elevated);
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
		padding: 6px 10px;
	}

	.code-box-block {
		flex-direction: column;
		align-items: flex-end;
		padding: 10px;
	}

	.code-box-block pre {
		width: 100%;
		margin: 0 0 8px 0;
		overflow-x: auto;
		font-family: var(--font-mono);
		font-size: 11px;
		line-height: 1.4;
		color: var(--text-primary);
	}

	.code-box code {
		font-family: var(--font-mono);
		font-size: 12px;
		color: var(--text-primary);
		overflow-x: auto;
		white-space: nowrap;
	}

	.copy-btn {
		background: var(--bg-surface);
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
		color: var(--text-secondary);
		font-family: var(--font-ui);
		font-size: 11px;
		padding: 3px 8px;
		cursor: pointer;
		white-space: nowrap;
	}

	.copy-btn:hover {
		background: var(--bg-hover);
		color: var(--text-primary);
	}

	.step-tip {
		font-size: 11px;
		color: var(--text-muted);
		margin: 0;
		line-height: 1.4;
	}

	.step-tip code,
	.guide-desc code,
	.section-desc code {
		font-family: var(--font-mono);
		color: var(--text-secondary);
	}

	.link-btn {
		background: none;
		border: none;
		color: var(--accent);
		text-decoration: underline;
		font-size: 11px;
		padding: 0;
		cursor: pointer;
	}

	.ordered-steps {
		margin: 0;
		padding-left: 18px;
		font-size: 12px;
		color: var(--text-secondary);
		line-height: 1.6;
	}

	.ordered-steps strong {
		color: var(--text-primary);
	}

	.ordered-steps code {
		font-family: var(--font-mono);
		background: var(--bg-elevated);
		padding: 1px 4px;
		border-radius: 3px;
	}

	.section-title {
		font-size: 13px;
		font-weight: 600;
		color: var(--text-primary);
		margin: 0 0 4px 0;
	}

	.section-desc {
		font-size: 12px;
		color: var(--text-secondary);
		margin: 0 0 14px 0;
	}

	.token-alert {
		background: var(--accent-15);
		border: 1px solid var(--accent);
		border-radius: var(--radius-sm);
		padding: 12px;
		margin-bottom: 14px;
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.token-alert-header {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.token-alert-title {
		font-size: 13px;
		font-weight: 600;
		color: var(--accent);
	}

	.token-alert-warn {
		font-size: 11px;
		color: var(--text-primary);
	}

	.raw-token {
		font-weight: 600;
		color: var(--accent);
	}

	.pat-form {
		margin-bottom: 18px;
	}

	.form-row {
		display: flex;
		gap: 10px;
		align-items: flex-end;
	}

	.form-field {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.form-field.flex-2 {
		flex: 2;
	}

	.form-field.flex-1 {
		flex: 1;
	}

	.form-field label {
		font-size: 11px;
		font-weight: 500;
		color: var(--text-secondary);
	}

	.form-field input,
	.form-field select {
		height: 30px;
		background: var(--bg-surface);
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
		padding: 0 8px;
		font-family: var(--font-ui);
		font-size: 12px;
		color: var(--text-primary);
	}

	.form-submit {
		height: 30px;
		padding: 0 12px;
		background: var(--accent);
		border: none;
		border-radius: var(--radius-sm);
		color: white;
		font-family: var(--font-ui);
		font-size: 12px;
		font-weight: 500;
		cursor: pointer;
		white-space: nowrap;
	}

	.form-submit:hover:not(:disabled) {
		background: var(--accent-hover);
	}

	.form-submit:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.sub-title {
		font-size: 12px;
		font-weight: 600;
		color: var(--text-secondary);
		margin: 0 0 8px 0;
		text-transform: uppercase;
		letter-spacing: 0.5px;
	}

	.empty-state {
		padding: 24px;
		text-align: center;
		color: var(--text-muted);
		font-size: 12px;
		background: var(--bg-surface);
		border: 1px dashed var(--border);
		border-radius: var(--radius-sm);
	}

	.tokens-table-container {
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
		overflow: hidden;
	}

	.tokens-table {
		width: 100%;
		border-collapse: collapse;
		font-size: 12px;
		text-align: left;
	}

	.tokens-table th {
		background: var(--bg-surface);
		padding: 8px 10px;
		font-weight: 600;
		color: var(--text-secondary);
		border-bottom: 1px solid var(--border);
	}

	.tokens-table td {
		padding: 8px 10px;
		border-bottom: 1px solid var(--border);
		color: var(--text-secondary);
	}

	.tokens-table tr:last-child td {
		border-bottom: none;
	}

	.cell-name strong {
		color: var(--text-primary);
	}

	.scope-badge {
		display: inline-block;
		background: var(--bg-hover);
		padding: 2px 6px;
		border-radius: 3px;
		font-family: var(--font-mono);
		font-size: 10px;
		color: var(--text-primary);
	}

	.type-badge {
		display: inline-block;
		background: var(--accent-15);
		color: var(--accent);
		padding: 2px 6px;
		border-radius: 3px;
		font-size: 10px;
		font-weight: 600;
	}

	.type-badge.type-pat {
		background: var(--bg-hover);
		color: var(--text-primary);
	}

	.revoke-btn {
		background: transparent;
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
		color: var(--error);
		padding: 2px 8px;
		font-size: 11px;
		cursor: pointer;
	}

	.revoke-btn:hover:not(:disabled) {
		background: var(--error-10);
	}

	.revoke-btn:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.section-header-flex {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: 12px;
	}

	.refresh-btn {
		background: var(--bg-surface);
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
		color: var(--text-secondary);
		padding: 4px 10px;
		font-size: 11px;
		cursor: pointer;
	}

	.refresh-btn:hover:not(:disabled) {
		background: var(--bg-hover);
		color: var(--text-primary);
	}
</style>
