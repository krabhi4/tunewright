<script lang="ts">
	import { untrack } from 'svelte';
	import type { FileEntry, TagData } from '$lib/types/audio';
	import { formatDuration, formatSize } from '$lib/utils/format';
	import {
		selectedIds,
		focusedId,
		toggleSelection,
		selectRange,
		directories,
		currentPath
	} from '$lib/stores/files';
	import { mergedTags, queueVisibleTagsFetch, pendingEdits, fetchTagsForFiles, queuePropertiesFetch } from '$lib/stores/tags';
	import { filterText, sortColumn, sortAsc } from '$lib/stores/ui';
	import ContextMenu from '$lib/components/common/ContextMenu.svelte';
	import { toast } from '$lib/stores/toast';
	import { copyText } from '$lib/utils/clipboard';

	interface Props {
		files: FileEntry[];
		onNavigate: (path: string) => void;
		filteredCount?: number;
		orderedIds?: string[];
	}

	let { files, onNavigate, filteredCount = $bindable(0), orderedIds = $bindable([]) }: Props = $props();

	// Virtual scrolling state
	let containerEl: HTMLDivElement;
	let gridEl: HTMLDivElement;
	let scrollTop = $state(0);
	const ROW_HEIGHT = 28;
	const HEADER_HEIGHT = 30;
	let containerHeight = $state(600);

	// Sort state is in $lib/stores/ui (synced to URL)

	// Last clicked row for shift-select
	let lastClickedId = $state<string | null>(null);

	// Context menu state
	let contextMenu = $state<{ x: number; y: number; file: FileEntry } | null>(null);

	function handleContextMenu(file: FileEntry, e: MouseEvent) {
		e.preventDefault();
		// Select the file if not already selected
		if (!$selectedIds.has(file.id)) {
			selectedIds.set(new Set([file.id]));
			lastClickedId = file.id;
			focusedId.set(file.id);
		}
		contextMenu = { x: e.clientX, y: e.clientY, file };
	}

	async function copyToClipboard(text: string, what: string) {
		if (await copyText(text)) toast.success(`${what} copied.`);
		else toast.error(`Could not copy ${what.toLowerCase()}.`);
	}

	function getContextMenuItems() {
		if (!contextMenu) return [];
		const file = contextMenu.file;
		return [
			{
				label: 'Copy Filename',
				action: () => copyToClipboard(file.filename, 'Filename')
			},
			{
				label: 'Copy Path',
				action: () => copyToClipboard(file.relative_path, 'Path')
			},
			{ separator: true as const },
			{
				label: `Select All (${processedFiles.length})`,
				action: () => {
					selectedIds.set(new Set(processedFiles.map((f) => f.id)));
				}
			}
		];
	}

	// Column definitions
	const columns = [
		{ key: 'filename', label: 'Filename', width: 200, mono: true },
		{ key: 'title', label: 'Title', width: 180, tag: true },
		{ key: 'artist', label: 'Artist', width: 150, tag: true },
		{ key: 'album', label: 'Album', width: 150, tag: true },
		{ key: 'year', label: 'Year', width: 50, tag: true, mono: true },
		{ key: 'track_number', label: '#', width: 36, tag: true, mono: true, align: 'right' as const },
		{ key: 'genre', label: 'Genre', width: 90, tag: true },
		{ key: 'format', label: 'Format', width: 50, mono: true },
		{ key: 'duration', label: 'Duration', width: 60, mono: true, align: 'right' as const },
		{ key: 'size', label: 'Size', width: 70, mono: true, align: 'right' as const }
	];

	// Sort keys whose values come from loaded tags (others come from the file entry)
	const tagSortKeys = new Set(['title', 'artist', 'album', 'year', 'track_number', 'genre', 'duration']);

	function getCellValue(file: FileEntry, tags: TagData | undefined, key: string): string {
		switch (key) {
			case 'filename':
				return file.filename;
			case 'format':
				return file.format_label;
			case 'duration':
				return formatDuration(tags?.duration_secs ?? file.duration_secs);
			case 'size':
				return formatSize(file.size);
			case 'title':
				return tags?.title ?? '';
			case 'artist':
				return tags?.artist ?? '';
			case 'album':
				return tags?.album ?? '';
			case 'year':
				return tags?.year != null ? String(tags.year) : '';
			case 'track_number':
				return tags?.track_number != null ? String(tags.track_number) : '';
			case 'genre':
				return tags?.genre ?? '';
			default:
				return '';
		}
	}

	function getSortValue(file: FileEntry, tags: TagData | undefined, key: string): string | number {
		switch (key) {
			case 'year':
				return tags?.year ?? -1;
			case 'track_number':
				return tags?.track_number ?? -1;
			case 'duration':
				return tags?.duration_secs ?? file.duration_secs ?? -1;
			case 'size':
				return file.size;
			default:
				return getCellValue(file, tags, key).toLowerCase();
		}
	}

	// Directories to show at the top of the grid
	let dirEntries = $derived($directories);

	const nameCollator = new Intl.Collator(undefined, { numeric: true, sensitivity: 'base' });

	// Filtered + sorted files
	let processedFiles = $derived.by(() => {
		let result = files;

		const q = $filterText.toLowerCase().trim();
		if (q) {
			result = result.filter((f) => {
				const tags = $mergedTags.get(f.id);
				return (
					f.filename.toLowerCase().includes(q) ||
					(tags?.title ?? '').toLowerCase().includes(q) ||
					(tags?.artist ?? '').toLowerCase().includes(q) ||
					(tags?.album ?? '').toLowerCase().includes(q) ||
					(tags?.genre ?? '').toLowerCase().includes(q)
				);
			});
		}

		if (!$sortColumn) {
			result = [...result].sort((a, b) => nameCollator.compare(a.filename, b.filename));
		} else {
			const col = $sortColumn;
			const dir = $sortAsc ? 1 : -1;
			const usesTags = tagSortKeys.has(col);
			// Precompute one sort key per file so the comparator doesn't redo the
			// map lookup + lowercasing O(n log n) times.
			const keyOf = new Map<string, string | number>();
			for (const f of result) {
				keyOf.set(f.id, getSortValue(f, usesTags ? $mergedTags.get(f.id) : undefined, col));
			}
			result = [...result].sort((a, b) => {
				const av = keyOf.get(a.id) ?? '';
				const bv = keyOf.get(b.id) ?? '';
				if (typeof av === 'string' && typeof bv === 'string') return nameCollator.compare(av, bv) * dir;
				if (av < bv) return -1 * dir;
				if (av > bv) return 1 * dir;
				return 0;
			});
		}

		return result;
	});

	// Expose filtered count to parent
	$effect(() => { filteredCount = processedFiles.length; });
	$effect(() => { orderedIds = processedFiles.map((f) => f.id); });
	$effect(() => {
		if (!$filterText.trim()) return;
		const visible = new Set(processedFiles.map((f) => f.id));
		const selected = untrack(() => $selectedIds);
		if ([...selected].some((id) => !visible.has(id))) {
			selectedIds.set(new Set([...selected].filter((id) => visible.has(id))));
		}
	});

	// Total rows = directories + files
	let visibleSelectedCount = $derived(processedFiles.filter((f) => $selectedIds.has(f.id)).length);
	let totalRows = $derived(dirEntries.length + processedFiles.length);

	// Virtual scroll calculations
	let visibleStart = $derived(Math.max(0, Math.floor(scrollTop / ROW_HEIGHT) - 2));
	let visibleCount = $derived(Math.ceil(containerHeight / ROW_HEIGHT) + 4);
	let visibleEnd = $derived(Math.min(totalRows, visibleStart + visibleCount));
	let totalHeight = $derived(totalRows * ROW_HEIGHT);
	let offsetY = $derived(visibleStart * ROW_HEIGHT);

	// Which visible rows are directories vs files
	type RowItem = { type: 'dir'; name: string } | { type: 'file'; file: FileEntry };

	let visibleRows = $derived.by(() => {
		const rows: RowItem[] = [];
		for (let i = visibleStart; i < visibleEnd; i++) {
			if (i < dirEntries.length) {
				rows.push({ type: 'dir', name: dirEntries[i] });
			} else {
				const fileIdx = i - dirEntries.length;
				if (fileIdx < processedFiles.length) {
					rows.push({ type: 'file', file: processedFiles[fileIdx] });
				}
			}
		}
		return rows;
	});

	let tabStopId = $derived.by(() => {
		const ids = visibleRows.flatMap((r) => (r.type === 'file' ? [r.file.id] : []));
		return $focusedId && ids.includes($focusedId) ? $focusedId : ids[0];
	});

	function handleFocus() {
		if (tabStopId && tabStopId !== $focusedId) focusedId.set(tabStopId);
	}

	// Fetch fast tags for visible file rows, then queue properties backfill
	$effect(() => {
		const fileIds = visibleRows
			.filter((r): r is { type: 'file'; file: FileEntry } => r.type === 'file')
			.map((r) => r.file.id);
		if (fileIds.length > 0) {
			queueVisibleTagsFetch(fileIds);
		}
	});

	$effect(() => {
		const col = $sortColumn;
		if (!$filterText.trim() && !(col && tagSortKeys.has(col))) return;
		const ids = files.map((f) => f.id);
		fetchTagsForFiles(ids).then(() => {
			if (col === 'duration') queuePropertiesFetch(ids);
		});
	});

	let headerEl: HTMLDivElement;

	function handleScroll() {
		scrollTop = containerEl.scrollTop;
		if (headerEl) {
			headerEl.scrollLeft = containerEl.scrollLeft;
		}
	}

	function handleSort(key: string) {
		if ($sortColumn === key) {
			sortAsc.set(!$sortAsc);
		} else {
			sortColumn.set(key);
			sortAsc.set(true);
		}
	}

	function rangeAnchor(): string | null {
		return processedFiles.some((f) => f.id === lastClickedId) ? lastClickedId : null;
	}

	function handleRowClick(file: FileEntry, e: MouseEvent | KeyboardEvent) {
		const anchor = rangeAnchor();
		if (e.shiftKey && anchor) {
			selectRange(anchor, file.id, processedFiles);
		} else {
			toggleSelection(file.id, e.ctrlKey || e.metaKey);
		}
		lastClickedId = file.id;
		focusedId.set(file.id);
		gridEl.focus();
	}

	function navigateToDir(dirName: string) {
		const base = $currentPath === '/' ? '' : $currentPath;
		const newPath = base + '/' + dirName;
		onNavigate(newPath);
	}

	function handleResize() {
		if (containerEl) {
			containerHeight = containerEl.clientHeight;
		}
	}

	$effect(() => {
		if (containerEl) {
			containerHeight = containerEl.clientHeight;
			const observer = new ResizeObserver(() => handleResize());
			observer.observe(containerEl);
			return () => observer.disconnect();
		}
	});

	function handleKeydown(e: KeyboardEvent) {
		if (processedFiles.length === 0 || headerEl.contains(e.target as Node)) return;

		const focIdx = $focusedId
			? processedFiles.findIndex((f) => f.id === $focusedId)
			: -1;

		if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'a') {
			e.preventDefault();
			selectedIds.set(new Set(processedFiles.map((f) => f.id)));
			return;
		}

		let nextIdx: number | null = null;
		const pageRows = Math.max(1, Math.floor(containerHeight / ROW_HEIGHT));

		switch (e.key) {
			case 'ArrowDown':
				e.preventDefault();
				nextIdx = focIdx < processedFiles.length - 1 ? focIdx + 1 : processedFiles.length - 1;
				break;
			case 'ArrowUp':
				e.preventDefault();
				nextIdx = focIdx > 0 ? focIdx - 1 : 0;
				break;
			case 'Home':
				e.preventDefault();
				nextIdx = 0;
				break;
			case 'End':
				e.preventDefault();
				nextIdx = processedFiles.length - 1;
				break;
			case 'PageUp':
				e.preventDefault();
				nextIdx = Math.max(0, focIdx - pageRows);
				break;
			case 'PageDown':
				e.preventDefault();
				nextIdx = Math.min(processedFiles.length - 1, focIdx + pageRows);
				break;
			case ' ':
				e.preventDefault();
				if ($focusedId) toggleSelection($focusedId, true);
				return;
			case 'Enter':
				if (e.target === gridEl && focIdx >= 0) handleRowClick(processedFiles[focIdx], e);
				return;
			default:
				return;
		}

		if (nextIdx !== null && nextIdx >= 0 && nextIdx < processedFiles.length) {
			const file = processedFiles[nextIdx];
			focusedId.set(file.id);

			const anchor = rangeAnchor();
			if (e.shiftKey && anchor) {
				selectRange(anchor, file.id, processedFiles);
			} else {
				// Select the focused row (replace selection)
				selectedIds.set(new Set([file.id]));
				lastClickedId = file.id;
			}

			// Scroll focused row into view
			const rowTop = (dirEntries.length + nextIdx) * ROW_HEIGHT;
			const rowBottom = rowTop + ROW_HEIGHT;
			if (containerEl) {
				if (rowTop < containerEl.scrollTop) {
					containerEl.scrollTop = rowTop;
				} else if (rowBottom > containerEl.scrollTop + containerHeight) {
					containerEl.scrollTop = rowBottom - containerHeight;
				}
				handleScroll();
			}
			gridEl.focus();
		}
	}
</script>

<div
	class="grid-wrapper"
	role="grid"
	aria-label="Audio files"
	aria-multiselectable="true"
	aria-rowcount={totalRows + 1}
	aria-activedescendant={tabStopId && tabStopId === $focusedId ? `row-${tabStopId}` : undefined}
	tabindex="0"
	bind:this={gridEl}
	onkeydown={handleKeydown}
	onfocus={handleFocus}
>
	<div class="grid-header" role="row" aria-rowindex={1} style="height: {HEADER_HEIGHT}px" bind:this={headerEl}>
		<div class="header-cell check-col" role="columnheader">
			<input
				type="checkbox"
				class="row-check"
				aria-label="Select all files"
				checked={processedFiles.length > 0 && visibleSelectedCount === processedFiles.length}
				indeterminate={visibleSelectedCount > 0 && visibleSelectedCount < processedFiles.length}
				onchange={() => {
					if (visibleSelectedCount === processedFiles.length) {
						selectedIds.set(new Set());
					} else {
						selectedIds.set(new Set(processedFiles.map((f) => f.id)));
					}
				}}
			/>
		</div>
		{#each columns as col}
			<div
				class="header-col"
				role="columnheader"
				aria-sort={$sortColumn === col.key ? ($sortAsc ? 'ascending' : 'descending') : 'none'}
				style="width: {col.width}px"
			>
				<button
					class="header-cell"
					class:sorted={$sortColumn === col.key}
					style={col.align === 'right' ? 'text-align: right; justify-content: flex-end;' : ''}
					onclick={() => handleSort(col.key)}
				>
					<span>{col.label}</span>
					{#if $sortColumn === col.key}
						<span class="sort-arrow">{$sortAsc ? '▲' : '▼'}</span>
					{/if}
				</button>
			</div>
		{/each}
		<div class="header-cell header-fill" aria-hidden="true"></div>
	</div>

	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div class="grid-body" bind:this={containerEl} onscroll={handleScroll}>
		{#if totalRows === 0}
			<div class="grid-empty">
				<svg class="empty-icon" aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M9 18V5l12-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/></svg>
				<span class="empty-title">No audio files</span>
				<span class="empty-hint">{$filterText ? 'Try a different filter' : 'Open a folder to get started. Use F3 to filter, Ctrl+S to save.'}</span>
			</div>
		{:else}
		<div style="height: {totalHeight}px; position: relative;">
			<div style="transform: translateY({offsetY}px);">
				{#each visibleRows as row, i (row.type === 'dir' ? `d:${row.name}` : `f:${row.file.id}`)}
					{#if row.type === 'dir'}
						<div
							class="grid-row dir-row"
							role="row"
							aria-rowindex={visibleStart + i + 2}
							tabindex="0"
							style="height: {ROW_HEIGHT}px"
							ondblclick={() => navigateToDir(row.name)}
							onkeydown={(e) => { if (e.key === 'Enter') { e.preventDefault(); navigateToDir(row.name); } }}
						>
							<div class="cell check-col" role="gridcell"></div>
							<div class="cell dir-cell" role="gridcell" style="width: {columns[0].width}px">
								<svg class="dir-icon" aria-hidden="true" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M2 4.5V12a1 1 0 001 1h10a1 1 0 001-1V6a1 1 0 00-1-1H8L6.5 3.5H3a1 1 0 00-1 1z"/></svg>
								{row.name}
							</div>
							{#each columns.slice(1) as col}
								<div class="cell" role="gridcell" style="width: {col.width}px"></div>
							{/each}
							<div class="cell cell-fill" aria-hidden="true"></div>
						</div>
					{:else}
						{@const file = row.file}
						{@const rowTags = $mergedTags.get(file.id)}
						{@const isSelected = $selectedIds.has(file.id)}
						{@const isOdd = (visibleStart + i) % 2 === 1}
						{@const isFocused = $focusedId === file.id}
						<div
							class="grid-row"
							class:selected={isSelected}
							class:odd={isOdd}
							class:focused={isFocused}
							class:dirty={$pendingEdits.has(file.id)}
							role="row"
							aria-rowindex={visibleStart + i + 2}
							aria-selected={isSelected}
							id="row-{file.id}"
							tabindex="-1"
							style="height: {ROW_HEIGHT}px"
							onclick={(e) => handleRowClick(file, e)}
							onkeydown={(e) => { if (e.key === 'Enter') handleRowClick(file, e); }}
							oncontextmenu={(e) => handleContextMenu(file, e)}
						>
							<div class="cell check-col" role="gridcell">
								<input
									type="checkbox"
									class="row-check"
									aria-label="Select {file.filename}"
									tabindex="-1"
									checked={isSelected}
									onclick={(e) => e.stopPropagation()}
									onkeydown={(e) => { if (e.key === ' ') e.stopPropagation(); }}
									onchange={() => { toggleSelection(file.id, true); focusedId.set(file.id); gridEl.focus(); }}
								/>
							</div>
							{#each columns as col}
								{@const val = getCellValue(file, rowTags, col.key)}
								<div
									class="cell"
									class:mono={col.mono}
									class:tag-cell={col.tag}
									role="gridcell"
									style="width: {col.width}px; {col.align === 'right' ? 'text-align: right; justify-content: flex-end;' : ''}"
									title={val}
								>
									{val}
								</div>
							{/each}
							<div class="cell cell-fill" aria-hidden="true"></div>
						</div>
					{/if}
				{/each}
			</div>
		</div>
		{/if}
	</div>
</div>

{#if contextMenu}
	<ContextMenu
		x={contextMenu.x}
		y={contextMenu.y}
		items={getContextMenuItems()}
		onClose={() => { contextMenu = null; gridEl.focus(); }}
	/>
{/if}

<style>
	.grid-wrapper {
		display: flex;
		flex-direction: column;
		height: 100%;
		overflow: hidden;
		background: var(--bg-base);
	}

	.grid-header {
		display: flex;
		background: var(--grid-header-bg);
		border-bottom: 1px solid var(--grid-border);
		flex-shrink: 0;
		user-select: none;
		overflow: hidden;
	}

	.header-cell {
		display: flex;
		align-items: center;
		gap: 4px;
		padding: 0 8px;
		font-size: 11px;
		font-weight: 500;
		color: var(--text-secondary);
		border: none;
		background: none;
		cursor: pointer;
		flex-shrink: 0;
		text-align: left;
		font-family: var(--font-ui);
		border-right: 1px solid var(--grid-border);
	}

	.header-col {
		display: flex;
		flex-shrink: 0;
	}

	.header-col .header-cell {
		flex: 1;
		min-width: 0;
	}

	.header-cell:hover {
		color: var(--text-primary);
		background: var(--bg-hover);
	}

	.header-cell.sorted {
		color: var(--accent);
	}

	.header-fill {
		flex: 1;
		cursor: default;
		border-right: none;
	}

	.header-fill:hover {
		background: transparent;
	}

	.sort-arrow {
		font-size: 8px;
	}

	.check-col {
		width: 32px;
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
		cursor: default;
	}

	.check-col:hover {
		background: transparent;
	}

	.row-check {
		width: 13px;
		height: 13px;
		accent-color: var(--accent);
		cursor: pointer;
	}

	.grid-body {
		flex: 1;
		overflow-y: auto;
		overflow-x: auto;
		outline: none;
	}

	.grid-row {
		display: flex;
		width: max-content;
		min-width: 100%;
		border: none;
		background: transparent;
		cursor: pointer;
		font-family: var(--font-ui);
		text-align: left;
		color: var(--text-primary);
		border-bottom: 1px solid var(--grid-border);
		user-select: none;
	}

	.grid-row.odd {
		background: var(--grid-row-alt);
	}

	.grid-row:hover {
		background: var(--grid-row-hover);
	}

	.grid-row.selected {
		background: var(--bg-selected);
	}

	.grid-row.selected:hover {
		background: var(--bg-selected-strong);
	}

	.grid-row.focused {
		outline: 1px solid var(--accent);
		outline-offset: -1px;
	}

	.grid-row.dirty {
		box-shadow: inset 2px 0 0 var(--state-dirty);
	}

	.grid-row.dirty .tag-cell {
		color: var(--state-dirty);
	}

	.grid-row.dir-row {
		background: var(--accent-subtle);
	}

	.grid-row.dir-row:hover {
		background: var(--bg-hover);
	}

	.dir-cell {
		font-weight: 500;
		display: flex;
		align-items: center;
		gap: 6px;
	}

	.dir-icon {
		width: 14px;
		height: 14px;
		flex-shrink: 0;
	}

	.cell {
		display: flex;
		align-items: center;
		padding: 0 8px;
		font-size: 12px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		flex-shrink: 0;
	}

	.cell.mono {
		font-family: var(--font-mono);
		font-size: 11.5px;
	}

	.cell.tag-cell {
		color: var(--text-primary);
	}

	.cell-fill {
		flex: 1;
	}

	.grid-empty {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 8px;
		height: 100%;
		padding: 40px 20px;
		user-select: none;
	}

	.empty-icon {
		width: 32px;
		height: 32px;
		color: var(--text-muted);
		opacity: 0.5;
		margin-bottom: 4px;
	}

	.empty-title {
		font-size: 13px;
		color: var(--text-secondary);
		font-weight: 500;
	}

	.empty-hint {
		font-size: 11px;
		color: var(--text-muted);
		text-align: center;
		max-width: 280px;
		line-height: 1.5;
	}

	@media (max-width: 768px) {
		.check-col {
			width: 44px;
		}

		.row-check {
			width: 18px;
			height: 18px;
		}

		.cell {
			font-size: 13px;
		}

		.cell.mono {
			font-size: 12px;
		}
	}
</style>
