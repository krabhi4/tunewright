<script lang="ts">
	import { goto } from '$app/navigation';
	import { apiFetch } from '$lib/api/client';

	let username = $state('');
	let password = $state('');
	let error = $state('');
	let loading = $state(false);

	async function handleLogin() {
		error = '';
		loading = true;
		try {
			await apiFetch('/auth/login', {
				method: 'POST',
				body: JSON.stringify({ username, password })
			});
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
			<div class="login-logo">T</div>
			<h1 class="login-title">TagStudio</h1>
		</div>

		<div class="login-form">
			<div class="field">
				<input
					type="text"
					bind:value={username}
					placeholder="Username"
					onkeydown={handleKeydown}
					class="login-input"
				/>
			</div>
			<div class="field">
				<input
					type="password"
					bind:value={password}
					placeholder="Password"
					onkeydown={handleKeydown}
					class="login-input"
				/>
			</div>

			{#if error}
				<div class="login-error">{error}</div>
			{/if}

			<button class="login-btn" onclick={handleLogin} disabled={loading}>
				{loading ? 'Signing in...' : 'Sign In'}
			</button>
		</div>
	</div>
</div>

<style>
	.login-page {
		height: 100vh;
		display: flex;
		align-items: center;
		justify-content: center;
		background: var(--bg-base);
	}

	.login-card {
		width: 320px;
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
		background: var(--bg-elevated);
		border-radius: 10px;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		font-size: 24px;
		font-weight: bold;
		color: var(--accent);
		margin-bottom: 12px;
	}

	.login-title {
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

	.login-input:focus {
		border-color: var(--accent);
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
		color: white;
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
</style>
