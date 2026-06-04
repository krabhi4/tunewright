<script lang="ts">
	import { goto } from '$app/navigation';
	import { setup } from '$lib/api/auth';
	import { auth } from '$lib/stores/auth';
	import Logo from '$lib/icons/Logo.svelte';

	let username = $state('');
	let password = $state('');
	let confirmPassword = $state('');
	let error = $state('');
	let loading = $state(false);

	async function handleSetup() {
		error = '';

		if (!username.trim()) {
			error = 'Username is required';
			return;
		}
		if (password.length < 8) {
			error = 'Password must be at least 8 characters';
			return;
		}
		if (password !== confirmPassword) {
			error = 'Passwords do not match';
			return;
		}

		loading = true;
		try {
			const result = await setup(username, password);
			if (result.user) {
				auth.set({ checked: true, setupRequired: false, authenticated: true, user: result.user });
			}
			goto('/');
		} catch (err: any) {
			error = err.message || 'Setup failed';
		} finally {
			loading = false;
		}
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter') handleSetup();
	}
</script>

<div class="login-page">
	<div class="login-card">
		<div class="login-header">
			<div class="login-logo">
				<Logo size={48} />
			</div>
			<h1 class="login-title">Welcome to Tunewright</h1>
			<p class="login-subtitle">Create your admin account to get started.</p>
		</div>

		<div class="login-form">
			<div class="field">
				<label for="setup-username" class="field-label">Username</label>
				<input
					id="setup-username"
					type="text"
					bind:value={username}
					placeholder="Username"
					autocomplete="username"
					required
					aria-required="true"
					aria-describedby={error ? 'setup-error' : undefined}
					onkeydown={handleKeydown}
					class="login-input"
				/>
			</div>
			<div class="field">
				<label for="setup-password" class="field-label">Password</label>
				<input
					id="setup-password"
					type="password"
					bind:value={password}
					placeholder="Password"
					autocomplete="new-password"
					required
					aria-required="true"
					aria-describedby={error ? 'setup-error' : undefined}
					onkeydown={handleKeydown}
					class="login-input"
				/>
			</div>
			<div class="field">
				<label for="setup-confirm" class="field-label">Confirm Password</label>
				<input
					id="setup-confirm"
					type="password"
					bind:value={confirmPassword}
					placeholder="Confirm Password"
					autocomplete="new-password"
					required
					aria-required="true"
					aria-describedby={error ? 'setup-error' : undefined}
					onkeydown={handleKeydown}
					class="login-input"
				/>
			</div>

			{#if error}
				<div id="setup-error" class="login-error" role="alert">{error}</div>
			{/if}

			<button class="login-btn" onclick={handleSetup} disabled={loading}>
				{loading ? 'Creating...' : 'Create Account'}
			</button>
		</div>
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
	.login-logo :global(svg) {
		width: 100%;
		height: 100%;
	}

	.login-title {
		font-size: 18px;
		font-weight: 600;
		color: var(--text-primary);
	}

	.login-subtitle {
		font-size: 12px;
		color: var(--text-secondary);
		margin-top: 6px;
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
</style>
