<script lang="ts">
	import '../app.css';
	import type { Snippet } from 'svelte';
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';

	let { children }: { children: Snippet } = $props();
	let authChecked = $state(false);

	onMount(async () => {
		try {
			const res = await fetch('/api/v1/auth/check');
			const data = await res.json();

			if (data.auth_required && !data.authenticated) {
				if (!page.url.pathname.startsWith('/login')) {
					goto('/login');
				}
			}
		} catch {
			// Server not reachable, proceed anyway
		} finally {
			authChecked = true;
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
