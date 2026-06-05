<script lang="ts">
	import '../app.css';
	import type { Snippet } from 'svelte';
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { checkAuth } from '$lib/api/auth';
	import { auth } from '$lib/stores/auth';
	import { initTheme } from '$lib/stores/theme';

	let { children }: { children: Snippet } = $props();
	let authChecked = $state(false);
	let serverError = $state(false);

	const authPages = ['/login', '/setup', '/register'];

	onMount(async () => {
		initTheme();
		try {
			const data = await checkAuth();

			if (data.setup_required) {
				auth.set({
					checked: true,
					setupRequired: true,
					setupTokenRequired: data.setup_token_required ?? false,
					authenticated: false,
					user: null
				});
			} else if (data.authenticated && data.user) {
				auth.set({ checked: true, setupRequired: false, authenticated: true, user: data.user });
			} else {
				auth.set({ checked: true, setupRequired: false, authenticated: false, user: null });
			}
			authChecked = true;
		} catch (err) {
			console.error('Auth check failed:', err);
			serverError = true;
		}
	});

	$effect(() => {
		if (authChecked) {
			const path = page.url.pathname;
			if ($auth.setupRequired) {
				if (!path.startsWith('/setup')) {
					goto('/setup');
				}
			} else if ($auth.authenticated) {
				if (authPages.some((p) => path.startsWith(p))) {
					goto('/');
				}
			} else {
				if (
					!path.startsWith('/login') &&
					!path.startsWith('/register')
				) {
					goto('/login');
				}
			}
		}
	});
</script>

<div class="app-root">
	{#if serverError}
		<div class="error-screen">
			<span>Server is unreachable. Please try again later.</span>
		</div>
	{:else if authChecked}
		{@render children()}
	{:else}
		<div class="loading-screen">
			<span>Loading...</span>
		</div>
	{/if}
</div>

<style>
	.app-root {
		width: 100vw;
		height: 100vh;
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}

	.loading-screen,
	.error-screen {
		display: flex;
		align-items: center;
		justify-content: center;
		height: 100%;
		font-size: 13px;
	}

	.loading-screen {
		color: var(--text-muted);
	}

	.error-screen {
		color: var(--text-error, #ff4d4f);
		font-weight: 500;
	}
</style>
