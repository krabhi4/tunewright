<script lang="ts">
	import { goto } from '$app/navigation';
	import { login } from '$lib/api/auth';
	import { auth } from '$lib/stores/auth';

	let username = $state('');
	let password = $state('');
	let error = $state('');
	let loading = $state(false);

	let setupRequired = $derived($auth.setupRequired);

	async function handleLogin() {
		error = '';
		loading = true;
		try {
			const result = await login(username, password);
			if (result.user) {
				auth.set({ checked: true, setupRequired: false, authenticated: true, user: result.user });
			}
			goto('/');
		} catch (err: any) {
			error = err.message || 'Login failed';
		} finally {
			loading = false;
		}
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter') handleLogin();
	}
</script>

<div class="login-page">
	<div class="login-card">
		<div class="login-header">
			<div class="login-logo">
				<svg class="tag-logo" viewBox="0 0 32 32" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
					<path d="M10 12 L16 6 L22 12 L22 25 A 2 2 0 0 1 20 27 L12 27 A 2 2 0 0 1 10 25 Z"/>
					<circle cx="16" cy="12" r="1.5" fill="currentColor"/>
					<rect x="12.25" y="16.5" width="1.5" height="6" rx="0.75" fill="currentColor"/>
					<rect x="15.25" y="14" width="1.5" height="10" rx="0.75" fill="currentColor"/>
					<rect x="18.25" y="17.5" width="1.5" height="5" rx="0.75" fill="currentColor"/>
				</svg>
			</div>
			<h1 class="login-title">TagStudio</h1>
		</div>

		<div class="login-form">
			<div class="field">
				<label for="login-username" class="field-label">Username</label>
				<input
					id="login-username"
					type="text"
					bind:value={username}
					placeholder="Username"
					autocomplete="username"
					required
					aria-required="true"
					aria-describedby={error ? 'login-error' : undefined}
					onkeydown={handleKeydown}
					class="login-input"
				/>
			</div>
			<div class="field">
				<label for="login-password" class="field-label">Password</label>
				<input
					id="login-password"
					type="password"
					bind:value={password}
					placeholder="Password"
					autocomplete="current-password"
					required
					aria-required="true"
					aria-describedby={error ? 'login-error' : undefined}
					onkeydown={handleKeydown}
					class="login-input"
				/>
			</div>

			{#if error}
				<div id="login-error" class="login-error" role="alert">{error}</div>
			{/if}

			<button class="login-btn" onclick={handleLogin} disabled={loading}>
				{loading ? 'Signing in...' : 'Sign In'}
			</button>
		</div>

		{#if setupRequired}
			<div class="login-footer">
				<p class="login-hint">No accounts exist yet.</p>
				<a href="/setup" class="login-link">Set up your admin account</a>
			</div>
		{/if}
	</div>
</div>

<style>
	.login-page {
		height: 100dvh;
		display: flex;
		align-items: center;
		justify-content: center;
		background: var(--bg-base);
	}

	.login-card {
		width: min(320px, calc(100vw - 32px));
		background: var(--bg-surface);
		border: 1px solid var(--border);
		border-radius: var(--radius-md);
		padding: 32px 28px;
	}

	.login-header {
		text-align: center;
		margin-bottom: 28px;
	}

	.login-logo {
		width: 48px;
		height: 48px;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		color: var(--accent);
		margin-bottom: 12px;
	}
	.login-logo :global(.tag-logo) {
		width: 100%;
		height: 100%;
	}

	.login-title {
		font-family: var(--font-display);
		font-size: 18px;
		font-weight: 600;
		color: var(--text-primary);
	}

	.login-form {
		display: flex;
		flex-direction: column;
		gap: 10px;
	}

	.field {
		display: flex;
		flex-direction: column;
	}

	.field-label {
		font-size: 11px;
		color: var(--text-secondary);
		margin-bottom: 4px;
	}

	.login-input {
		width: 100%;
		background: var(--bg-base);
		border: 1px solid var(--border);
		color: var(--text-primary);
		font-family: var(--font-ui);
		font-size: 13px;
		padding: 8px 10px;
		border-radius: var(--radius-sm);
		outline: none;
	}

	.login-input:focus-visible {
		border-color: var(--accent);
		box-shadow: 0 0 0 1px var(--accent);
	}

	.login-input::placeholder {
		color: var(--text-placeholder);
	}

	.login-error {
		color: var(--error);
		font-size: 12px;
		text-align: center;
	}

	.login-btn {
		width: 100%;
		background: var(--accent);
		border: none;
		color: var(--text-on-accent);
		padding: 9px;
		border-radius: var(--radius-sm);
		cursor: pointer;
		font-family: var(--font-ui);
		font-size: 13px;
		font-weight: 500;
		margin-top: 4px;
	}

	.login-btn:hover:not(:disabled) {
		background: var(--accent-hover);
	}

	.login-btn:disabled {
		opacity: 0.5;
	}

	.login-footer {
		text-align: center;
		margin-top: 16px;
	}

	.login-hint {
		color: var(--text-muted);
		font-size: 12px;
		margin-bottom: 4px;
	}

	.login-link {
		color: var(--accent);
		font-size: 12px;
		text-decoration: none;
	}

	.login-link:hover {
		color: var(--accent-hover);
	}
</style>
