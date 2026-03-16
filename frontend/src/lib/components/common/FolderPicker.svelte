<script lang="ts">
	import Modal from './Modal.svelte';
	import type { DirNode } from '$lib/types/audio';
	import { getDirTree } from '$lib/api/files';

	interface Props {
		open: boolean;
		onClose: () => void;
		onSelect: (path: string) => void;
	}

	let { open, onClose, onSelect }: Props = $props();

	let tree: DirNode | null = $state(null);
	let loading = $state(false);
	let expandedPaths = $state(new Set<string>(['/']));

	$effect(() => {
		if (open && !tree) {
			loadTree();
		}
	});

	async function loadTree() {
		loading = true;
		try {
			tree = await getDirTree(4);
			// Auto-expand root
			if (tree) expandedPaths.add('');
		} catch (err) {
			console.error('Failed to load directory tree:', err);
		} finally {
			loading = false;
		}
	}

	function toggleExpand(path: string) {
		const next = new Set(expandedPaths);
		if (next.has(path)) {
			next.delete(path);
		} else {
			next.add(path);
		}
		expandedPaths = next;
	}

	function handleSelect(path: string) {
		onSelect(path || '/');
		onClose();
	}
</script>

<Modal title="Open Folder" {open} {onClose}>
	{#if loading}
		<div class="loading">Loading directories...</div>
	{:else if tree}
		<div class="tree">
			{@render treeNode(tree, 0)}
		</div>
	{:else}
		<div class="loading">No directories found</div>
	{/if}
</Modal>

{#snippet treeNode(node: DirNode, depth: number)}
	<div class="tree-item" style="padding-left: {depth * 16}px">
		<button
			class="tree-toggle"
			onclick={() => toggleExpand(node.path)}
			class:has-children={node.children.length > 0}
		>
			{#if node.children.length > 0}
				<span class="arrow" class:expanded={expandedPaths.has(node.path)}>&#9654;</span>
			{:else}
				<span class="arrow-space"></span>
			{/if}
		</button>
		<button class="tree-label" ondblclick={() => handleSelect(node.path)}>
			<span class="folder-icon">&#128193;</span>
			{node.name || 'Root'}
		</button>
		<button class="tree-select" onclick={() => handleSelect(node.path)}>Open</button>
	</div>
	{#if expandedPaths.has(node.path) && node.children.length > 0}
		{#each node.children as child}
			{@render treeNode(child, depth + 1)}
		{/each}
	{/if}
{/snippet}

<style>
	.tree {
		max-height: 400px;
		overflow-y: auto;
	}

	.tree-item {
		display: flex;
		align-items: center;
		height: 28px;
		gap: 2px;
	}

	.tree-item:hover .tree-select {
		opacity: 1;
	}

	.tree-toggle {
		background: none;
		border: none;
		cursor: pointer;
		padding: 0;
		width: 18px;
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
	}

	.tree-toggle:not(.has-children) {
		cursor: default;
	}

	.arrow {
		font-size: 8px;
		color: var(--text-muted);
		transition: transform 0.12s;
		display: inline-block;
	}

	.arrow.expanded {
		transform: rotate(90deg);
	}

	.arrow-space {
		width: 8px;
	}

	.tree-label {
		flex: 1;
		background: none;
		border: none;
		color: var(--text-primary);
		font-family: var(--font-ui);
		font-size: 12.5px;
		text-align: left;
		cursor: pointer;
		padding: 2px 4px;
		border-radius: var(--radius-sm);
		display: flex;
		align-items: center;
		gap: 5px;
	}

	.tree-label:hover {
		background: var(--bg-hover);
	}

	.folder-icon {
		font-size: 13px;
	}

	.tree-select {
		font-size: 10px;
		color: var(--accent);
		background: var(--accent-subtle);
		border: none;
		padding: 2px 8px;
		border-radius: var(--radius-sm);
		cursor: pointer;
		opacity: 0;
		transition: opacity 0.1s;
		flex-shrink: 0;
		font-family: var(--font-ui);
	}

	.tree-select:hover {
		background: var(--accent-muted);
	}

	.loading {
		color: var(--text-muted);
		font-size: 12px;
		padding: 20px;
		text-align: center;
	}
</style>
