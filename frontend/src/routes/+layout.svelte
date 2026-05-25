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

	const authPages = ['/login', '/setup', '/register'];

	onMount(async () => {
		initTheme();
		try {
			const data = await checkAuth();

			if (data.setup_required) {
				auth.set({ checked: true, setupRequired: true, authenticated: false, user: null });
			} else if (data.authenticated && data.user) {
				auth.set({ checked: true, setupRequired: false, authenticated: true, user: data.user });
			} else {
				auth.set({ checked: true, setupRequired: false, authenticated: false, user: null });
			}
		} catch {
			auth.set({ checked: true, setupRequired: false, authenticated: false, user: null });
		} finally {
			authChecked = true;
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
	{#if authChecked}
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

	.loading-screen {
		display: flex;
		align-items: center;
		justify-content: center;
		height: 100%;
		color: var(--text-muted);
		font-size: 13px;
	}
</style>
