<script lang="ts">
	import Toolbar from '$lib/components/layout/Toolbar.svelte';
	import StatusBar from '$lib/components/layout/StatusBar.svelte';
	import FilterBar from '$lib/components/layout/FilterBar.svelte';
	import PathBar from '$lib/components/layout/PathBar.svelte';
	import TagPanel from '$lib/components/tagpanel/TagPanel.svelte';
	import FileGrid from '$lib/components/grid/FileGrid.svelte';
	import FolderPicker from '$lib/components/common/FolderPicker.svelte';
	import RenameModal from '$lib/components/rename/RenameModal.svelte';
	import LookupModal from '$lib/components/lookup/LookupModal.svelte';
	import {
		files,
		totalCount,
		selectedCount,
		loading,
		loadDirectory,
		currentPath,
		selectedFiles
	} from '$lib/stores/files';
	import { filterVisible, sidebarWidth, sidebarCollapsed } from '$lib/stores/ui';
	import {
		clearTags,
		fetchTagsForFiles,
		hasPendingEdits,
		saveAllEdits,
		pendingEdits
	} from '$lib/stores/tags';
	import { selectedIds } from '$lib/stores/files';

	let folderPickerOpen = $state(false);
	let renameModalOpen = $state(false);
	let lookupModalOpen = $state(false);

	async function handleSave() {
		const result = await saveAllEdits();
		if (result.failed > 0) {
			console.warn(`Save: ${result.success} ok, ${result.failed} failed`);
		}
	}

	// Resizable sidebar
	let isResizing = $state(false);

	function startResize(e: MouseEvent) {
		isResizing = true;
		const startX = e.clientX;
		const startWidth = $sidebarWidth;

		function onMove(e: MouseEvent) {
			const delta = e.clientX - startX;
			const newWidth = Math.max(200, Math.min(400, startWidth + delta));
			sidebarWidth.set(newWidth);
		}

		function onUp() {
			isResizing = false;
			document.removeEventListener('mousemove', onMove);
			document.removeEventListener('mouseup', onUp);
		}

		document.addEventListener('mousemove', onMove);
		document.addEventListener('mouseup', onUp);
	}

	// Computed stats
	let totalDuration = $derived(
		$files.reduce((sum, f) => sum + (f.duration_secs ?? 0), 0)
	);
	let totalSize = $derived($files.reduce((sum, f) => sum + f.size, 0));
	let modifiedCount = $derived($pendingEdits.size);

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'F3') {
			e.preventDefault();
			filterVisible.update((v) => !v);
		}
		if ((e.ctrlKey || e.metaKey) && e.key === 's') {
			e.preventDefault();
			if ($hasPendingEdits) handleSave();
		}
	}

	// Fetch tags for newly selected files
	$effect(() => {
		const ids = Array.from($selectedIds);
		if (ids.length > 0) {
			fetchTagsForFiles(ids);
		}
	});

	// Load root on mount
	import { onMount } from 'svelte';
	onMount(() => {
		loadDirectory('/');
	});
</script>

<svelte:window onkeydown={handleKeydown} />

<Toolbar
	onOpenFolder={() => (folderPickerOpen = true)}
	onSave={handleSave}
	onRename={() => (renameModalOpen = true)}
	onLookup={() => (lookupModalOpen = true)}
	hasPendingEdits={$hasPendingEdits}
	hasSelection={$selectedCount > 0}
/>

<PathBar />

<FilterBar />

<div class="main-content" class:resizing={isResizing}>
	{#if !$sidebarCollapsed}
		<div class="sidebar" style="width: {$sidebarWidth}px">
			<TagPanel />
		</div>
		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<div class="resize-handle" onmousedown={startResize}></div>
	{/if}

	<div class="content-area">
		{#if $loading}
			<div class="loading-overlay">
				<span class="loading-text">Scanning files...</span>
			</div>
		{/if}

		<FileGrid files={$files} />
	</div>
</div>

<StatusBar
	fileCount={$totalCount}
	selectedCount={$selectedCount}
	{totalDuration}
	{totalSize}
	{modifiedCount}
/>

<FolderPicker
	open={folderPickerOpen}
	onClose={() => (folderPickerOpen = false)}
	onSelect={(path) => { clearTags(); loadDirectory(path); }}
/>

<RenameModal
	open={renameModalOpen}
	onClose={() => (renameModalOpen = false)}
	files={$selectedFiles}
	onComplete={() => { clearTags(); loadDirectory($currentPath); }}
/>

<LookupModal
	open={lookupModalOpen}
	onClose={() => (lookupModalOpen = false)}
/>

<style>
	.main-content {
		flex: 1;
		display: flex;
		overflow: hidden;
		position: relative;
	}

	.main-content.resizing {
		cursor: col-resize;
		user-select: none;
	}

	.sidebar {
		flex-shrink: 0;
		overflow: hidden;
	}

	.resize-handle {
		width: 3px;
		cursor: col-resize;
		background: transparent;
		flex-shrink: 0;
		transition: background 0.15s;
	}

	.resize-handle:hover {
		background: var(--accent-muted);
	}

	.content-area {
		flex: 1;
		overflow: hidden;
		position: relative;
	}

	.loading-overlay {
		position: absolute;
		inset: 0;
		display: flex;
		align-items: center;
		justify-content: center;
		background: rgba(15, 15, 26, 0.7);
		z-index: 10;
	}

	.loading-text {
		color: var(--text-secondary);
		font-size: 13px;
	}
</style>
