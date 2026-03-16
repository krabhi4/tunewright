<script lang="ts">
	import { currentPath, loadDirectory } from '$lib/stores/files';
	import { clearTags } from '$lib/stores/tags';

	// Split current path into clickable breadcrumb segments
	let segments = $derived.by(() => {
		const path = $currentPath;
		if (!path || path === '/') return [{ name: 'Root', path: '/' }];
		const parts = path.split('/').filter(Boolean);
		const crumbs = [{ name: 'Root', path: '/' }];
		let accumulated = '';
		for (const part of parts) {
			accumulated += '/' + part;
			crumbs.push({ name: part, path: accumulated });
		}
		return crumbs;
	});

	function navigate(path: string) {
		clearTags();
		loadDirectory(path);
	}
</script>

<div class="pathbar">
	{#each segments as seg, i}
		{#if i > 0}
			<span class="sep">/</span>
		{/if}
		{#if i === segments.length - 1}
			<span class="segment current">{seg.name}</span>
		{:else}
			<button class="segment link" onclick={() => navigate(seg.path)}>
				{seg.name}
			</button>
		{/if}
	{/each}
</div>

<style>
	.pathbar {
		height: 30px;
		background: var(--bg-surface);
		border-bottom: 1px solid var(--border);
		display: flex;
		align-items: center;
		padding: 0 10px;
		gap: 0;
		flex-shrink: 0;
		overflow-x: auto;
		white-space: nowrap;
		user-select: none;
	}

	.sep {
		color: var(--text-muted);
		font-size: 11px;
		margin: 0 3px;
	}

	.segment {
		font-size: 12px;
		font-family: var(--font-mono);
	}

	.segment.current {
		color: var(--text-primary);
		font-weight: 500;
	}

	.segment.link {
		color: var(--text-secondary);
		background: none;
		border: none;
		cursor: pointer;
		padding: 2px 4px;
		border-radius: var(--radius-sm);
		font-family: var(--font-mono);
		font-size: 12px;
	}

	.segment.link:hover {
		color: var(--accent);
		background: var(--accent-subtle);
	}
</style>
