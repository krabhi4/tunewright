<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { register } from '$lib/api/auth';
	import { auth } from '$lib/stores/auth';

	let token = $state(page.url.searchParams.get('token') || '');
	let username = $state('');
	let password = $state('');
	let confirmPassword = $state('');
	let error = $state('');
	let loading = $state(false);

	async function handleRegister() {
		error = '';

		if (!token.trim()) {
			error = 'Invite token is missing';
			return;
		}
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
			const result = await register(token, username, password);
			if (result.user) {
				auth.set({ checked: true, setupRequired: false, authenticated: true, user: result.user });
			}
			goto('/');
		} catch (err: any) {
			error = err.message || 'Registration failed';
		} finally {
			loading = false;
		}
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter') handleRegister();
	}
</script>

<div class="login-page">
	<div class="login-card">
		<div class="login-header">
			<div class="login-logo">T</div>
			<h1 class="login-title">Join TagStudio</h1>
			<p class="login-subtitle">You've been invited. Create your account.</p>
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
			<div class="field">
				<input
					type="password"
					bind:value={confirmPassword}
					placeholder="Confirm Password"
					onkeydown={handleKeydown}
					class="login-input"
				/>
			</div>

			{#if error}
				<div class="login-error">{error}</div>
			{/if}

			<button class="login-btn" onclick={handleRegister} disabled={loading}>
				{loading ? 'Creating...' : 'Create Account'}
			</button>
		</div>

		<div class="login-footer">
			<a href="/login" class="login-link">Already have an account? Sign in</a>
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

	.login-footer {
		text-align: center;
		margin-top: 16px;
	}

	.login-link {
		color: var(--text-secondary);
		font-size: 12px;
		text-decoration: none;
	}

	.login-link:hover {
		color: var(--accent);
	}
</style>
