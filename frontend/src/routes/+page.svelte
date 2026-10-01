<script lang="ts">
	import Toolbar from '$lib/components/layout/Toolbar.svelte';
	import StatusBar from '$lib/components/layout/StatusBar.svelte';
	import FilterBar from '$lib/components/layout/FilterBar.svelte';
	import PathBar from '$lib/components/layout/PathBar.svelte';
	import TagPanel from '$lib/components/tagpanel/TagPanel.svelte';
	import FileGrid from '$lib/components/grid/FileGrid.svelte';
	import FolderPicker from '$lib/components/common/FolderPicker.svelte';
	import ConfirmModal from '$lib/components/common/ConfirmModal.svelte';
	import RenameModal from '$lib/components/rename/RenameModal.svelte';
	import FilenameToTagModal from '$lib/components/filename2tag/FilenameToTagModal.svelte';
	import ActionsModal from '$lib/components/actions/ActionsModal.svelte';
	import LookupModal from '$lib/components/lookup/LookupModal.svelte';
	import UserManagementModal from '$lib/components/layout/UserManagementModal.svelte';
	import {
		files,
		totalCount,
		selectedCount,
		loading,
		loadDirectory,
		currentPath,
		selectedFiles
	} from '$lib/stores/files';
	import { filterVisible, filterText, sidebarWidth, sidebarCollapsed, sortColumn, sortAsc } from '$lib/stores/ui';
	import {
		clearTags,
		discardEdits,
		fetchTagsForFiles,
		hasPendingEdits,
		saveAllEdits,
		pendingEditCount
	} from '$lib/stores/tags';
	import { selectedIds } from '$lib/stores/files';
	import { toast } from '$lib/stores/toast';
	import { onMount } from 'svelte';
	import { get } from 'svelte/store';
	import { goto } from '$app/navigation';

	let isNarrow = $state(false);
	let filteredCount = $state(0);

	$effect(() => {
		const mq = window.matchMedia('(max-width: 768px)');
		isNarrow = mq.matches;
		function onChange(e: MediaQueryListEvent) { isNarrow = e.matches; }
		mq.addEventListener('change', onChange);
		return () => mq.removeEventListener('change', onChange);
	});

	let folderPickerOpen = $state(false);
	let renameModalOpen = $state(false);
	let filenameToTagOpen = $state(false);
	let actionsModalOpen = $state(false);
	let lookupModalOpen = $state(false);
	let userManagementOpen = $state(false);

	function closeAllModals() {
		folderPickerOpen = false;
		renameModalOpen = false;
		filenameToTagOpen = false;
		actionsModalOpen = false;
		lookupModalOpen = false;
		userManagementOpen = false;
	}

	// --- Unsaved edits guard ---
	let confirmOpen = $state(false);
	let pendingAction = $state<(() => void) | null>(null);
	let saving = $state(false);

	function guardUnsaved(action: () => void) {
		if ($hasPendingEdits) {
			pendingAction = action;
			confirmOpen = true;
		} else {
			action();
		}
	}

	function navigateTo(path: string) {
		guardUnsaved(() => doNavigate(path));
	}

	function openModal(show: () => void) {
		closeAllModals();
		guardUnsaved(show);
	}

	function doNavigate(path: string) {
		clearTags();
		loadDirectory(path);
	}

	function reloadKeepingEdits() {
		clearTags(true);
		loadDirectory($currentPath);
	}

	async function handleConfirmSave() {
		if (saving) return;
		saving = true;
		const result = await saveAllEdits().finally(() => (saving = false));
		const action = pendingAction;
		pendingAction = null;
		confirmOpen = false;
		if (result.failed > 0) {
			toast.error(`Failed to save edits for ${result.failed} file(s). Action cancelled.`);
			return;
		}
		action?.();
	}

	function handleConfirmDiscard() {
		confirmOpen = false;
		discardEdits();
		pendingAction?.();
		pendingAction = null;
	}

	function handleConfirmCancel() {
		confirmOpen = false;
		pendingAction = null;
	}

	$effect(() => {
		if (!$hasPendingEdits) return;
		function onBeforeUnload(e: BeforeUnloadEvent) {
			e.preventDefault();
		}
		window.addEventListener('beforeunload', onBeforeUnload);
		return () => window.removeEventListener('beforeunload', onBeforeUnload);
	});

	// --- URL state sync ---
	let initialized = false;

	onMount(() => {
		const params = new URLSearchParams(window.location.search);
		const urlPath = params.get('path') || '/';
		const urlFilter = params.get('filter') || '';
		const urlSort = params.get('sort') || '';
		const urlOrder = params.get('order') || 'asc';
		const urlSelected = params.get('selected') || '';

		if (urlFilter) {
			filterText.set(urlFilter);
			filterVisible.set(true);
		}
		if (urlSort) {
			sortColumn.set(urlSort);
			sortAsc.set(urlOrder !== 'desc');
		}

		const pendingSelection = urlSelected ? urlSelected.split(',') : [];

		loadDirectory(urlPath).then(() => {
			if (pendingSelection.length > 0) {
				const loadedFileIds = new Set(get(files).map((f) => f.id));
				const validIds = pendingSelection.filter((id) => loadedFileIds.has(id));
				if (validIds.length > 0) {
					selectedIds.set(new Set(validIds));
				}
			}
		});
		initialized = true;
	});

	function syncToUrl() {
		if (!initialized) return;
		const params = new URLSearchParams();

		const path = $currentPath;
		if (path && path !== '/') params.set('path', path);

		const filter = $filterText;
		if (filter) params.set('filter', filter);

		const sort = $sortColumn;
		if (sort) {
			params.set('sort', sort);
			if (!$sortAsc) params.set('order', 'desc');
		}

		const ids = Array.from($selectedIds);
		if (ids.length > 0 && ids.length <= 50) {
			params.set('selected', ids.join(','));
		}

		const qs = params.toString();
		const newUrl = qs ? `?${qs}` : window.location.pathname;

		if (newUrl !== window.location.pathname + window.location.search) {
			history.replaceState(history.state, '', newUrl);
		}
	}

	$effect(() => {
		$currentPath;
		$filterText;
		$sortColumn;
		$sortAsc;
		$selectedIds;
		const timer = setTimeout(syncToUrl, 250);
		return () => clearTimeout(timer);
	});

	let activeSave: ReturnType<typeof saveAllEdits> | null = null;

	async function handleSave() {
		const save = saveAllEdits();
		if (save === activeSave) return;
		activeSave = save;
		saving = true;
		const result = await save.finally(() => {
			if (activeSave !== save) return;
			activeSave = null;
			saving = false;
		});
		if (result.failed > 0) {
			const saved = result.success > 0 ? `; ${result.success} saved` : '';
			toast.error(`Failed to save edits for ${result.failed} file(s)${saved}.`);
		} else if (result.success > 0) {
			toast.success(`Saved ${result.success} file(s).`);
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

	// Single pass over the file list for both aggregates.
	let totals = $derived.by(() => {
		let duration = 0;
		let size = 0;
		for (const f of $files) {
			duration += f.duration_secs ?? 0;
			size += f.size;
		}
		return { duration, size };
	});

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'F3') {
			e.preventDefault();
			filterVisible.update((v) => !v);
		}
		if ((e.ctrlKey || e.metaKey) && e.key === 's') {
			e.preventDefault();
			const active = document.activeElement as HTMLElement | null;
			active?.blur();
			active?.focus();
			if (get(hasPendingEdits)) handleSave();
		}
	}

	$effect(() => {
		const ids = Array.from($selectedIds);
		if (ids.length > 0) {
			fetchTagsForFiles(ids);
		}
	});
</script>

<svelte:window onkeydown={handleKeydown} />

<Toolbar
	onOpenFolder={() => { closeAllModals(); folderPickerOpen = true; }}
	onSave={handleSave}
	onRename={() => openModal(() => (renameModalOpen = true))}
	onFilenameToTag={() => openModal(() => (filenameToTagOpen = true))}
	onActions={() => openModal(() => (actionsModalOpen = true))}
	onLookup={() => openModal(() => (lookupModalOpen = true))}
	onManageUsers={() => { closeAllModals(); userManagementOpen = true; }}
	hasPendingEdits={$hasPendingEdits && !saving}
	hasSelection={$selectedCount > 0}
/>

<PathBar onNavigate={navigateTo} />

<FilterBar matchCount={filteredCount} totalCount={$files.length} />

<div class="main-content" class:resizing={isResizing}>
	{#if !$sidebarCollapsed && !isNarrow}
		<div class="sidebar" style="width: {$sidebarWidth}px">
			<TagPanel />
		</div>
		<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
		<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
		<div
			class="resize-handle"
			role="separator"
			aria-orientation="vertical"
			aria-label="Resize sidebar"
			tabindex="0"
			onmousedown={startResize}
			onkeydown={(e) => {
				const step = e.shiftKey ? 20 : 5;
				if (e.key === 'ArrowLeft') {
					e.preventDefault();
					sidebarWidth.set(Math.max(200, $sidebarWidth - step));
				} else if (e.key === 'ArrowRight') {
					e.preventDefault();
					sidebarWidth.set(Math.min(400, $sidebarWidth + step));
				}
			}}
		></div>
	{/if}

	<div class="content-area">
		{#if $loading}
			<div class="loading-overlay">
				<div class="spinner"></div>
				<span class="loading-text">Scanning files...</span>
			</div>
		{/if}

		<FileGrid files={$files} onNavigate={navigateTo} bind:filteredCount />
	</div>
</div>

<StatusBar
	fileCount={$totalCount}
	selectedCount={$selectedCount}
	totalDuration={totals.duration}
	totalSize={totals.size}
	modifiedCount={$pendingEditCount}
/>

<FolderPicker
	open={folderPickerOpen}
	onClose={() => (folderPickerOpen = false)}
	onSelect={navigateTo}
/>

<RenameModal
	open={renameModalOpen}
	onClose={() => (renameModalOpen = false)}
	files={$selectedFiles}
	onComplete={reloadKeepingEdits}
/>

<FilenameToTagModal
	open={filenameToTagOpen}
	onClose={() => (filenameToTagOpen = false)}
	files={$selectedFiles}
	onComplete={() => {}}
/>

<ActionsModal
	open={actionsModalOpen}
	onClose={() => (actionsModalOpen = false)}
	files={$selectedFiles}
	onComplete={() => {}}
/>

<LookupModal
	open={lookupModalOpen}
	onClose={() => (lookupModalOpen = false)}
	onApplied={reloadKeepingEdits}
/>

<ConfirmModal
	open={confirmOpen}
	title="Unsaved Changes"
	message="You have unsaved tag edits. What would you like to do?"
	confirmLabel="Save & Continue"
	extraLabel="Discard & Continue"
	cancelLabel="Cancel"
	busy={saving}
	onConfirm={handleConfirmSave}
	onExtra={handleConfirmDiscard}
	onCancel={handleConfirmCancel}
/>

<UserManagementModal
	open={userManagementOpen}
	onClose={() => (userManagementOpen = false)}
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
		gap: 8px;
		background: var(--backdrop);
		z-index: 10;
	}

	.loading-text {
		color: var(--text-secondary);
		font-size: 13px;
	}
</style>
