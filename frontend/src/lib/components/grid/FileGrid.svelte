<script lang="ts">
	import type { FileEntry } from '$lib/types/audio';
	import { formatDuration, formatSize, formatFormatLabel } from '$lib/utils/format';
	import { selectedIds, toggleSelection, selectRange } from '$lib/stores/files';
	import { mergedTags, fetchTagsForFiles } from '$lib/stores/tags';
	import { filterText } from '$lib/stores/ui';

	interface Props {
		files: FileEntry[];
	}

	let { files }: Props = $props();

	// Virtual scrolling state
	let containerEl: HTMLDivElement;
	let scrollTop = $state(0);
	const ROW_HEIGHT = 28;
	const HEADER_HEIGHT = 30;
	let containerHeight = $state(600);

	// Sort state
	let sortColumn = $state<string | null>(null);
	let sortAsc = $state(true);

	// Last clicked row for shift-select
	let lastClickedId = $state<string | null>(null);

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

	function getCellValue(file: FileEntry, key: string): string {
		const tags = $mergedTags.get(file.id);
		switch (key) {
			case 'filename': return file.filename;
			case 'format': return formatFormatLabel(file.format);
			case 'duration': return formatDuration(file.duration_secs);
			case 'size': return formatSize(file.size);
			case 'title': return tags?.title ?? '';
			case 'artist': return tags?.artist ?? '';
			case 'album': return tags?.album ?? '';
			case 'year': return tags?.year != null ? String(tags.year) : '';
			case 'track_number': return tags?.track_number != null ? String(tags.track_number) : '';
			case 'genre': return tags?.genre ?? '';
			default: return '';
		}
	}

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

		if (sortColumn) {
			const col = sortColumn;
			const dir = sortAsc ? 1 : -1;
			result = [...result].sort((a, b) => {
				const av = getCellValue(a, col).toLowerCase();
				const bv = getCellValue(b, col).toLowerCase();
				if (av < bv) return -1 * dir;
				if (av > bv) return 1 * dir;
				return 0;
			});
		}

		return result;
	});

	// Virtual scroll calculations
	let visibleStart = $derived(Math.max(0, Math.floor(scrollTop / ROW_HEIGHT) - 2));
	let visibleCount = $derived(Math.ceil(containerHeight / ROW_HEIGHT) + 4);
	let visibleEnd = $derived(Math.min(processedFiles.length, visibleStart + visibleCount));
	let visibleFiles = $derived(processedFiles.slice(visibleStart, visibleEnd));
	let totalHeight = $derived(processedFiles.length * ROW_HEIGHT);
	let offsetY = $derived(visibleStart * ROW_HEIGHT);

	// Fetch tags for visible files
	$effect(() => {
		const ids = visibleFiles.map((f) => f.id);
		if (ids.length > 0) {
			fetchTagsForFiles(ids);
		}
	});

	function handleScroll() {
		scrollTop = containerEl.scrollTop;
	}

	function handleSort(key: string) {
		if (sortColumn === key) {
			sortAsc = !sortAsc;
		} else {
			sortColumn = key;
			sortAsc = true;
		}
	}

	function handleRowClick(file: FileEntry, e: MouseEvent) {
		if (e.shiftKey && lastClickedId) {
			selectRange(lastClickedId, file.id);
		} else {
			toggleSelection(file.id, e.ctrlKey || e.metaKey);
		}
		lastClickedId = file.id;
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
</script>

<div class="grid-wrapper">
	<div class="grid-header" style="height: {HEADER_HEIGHT}px">
		<div class="header-cell check-col">
			<input type="checkbox" class="row-check" />
		</div>
		{#each columns as col}
			<button
				class="header-cell"
				class:sorted={sortColumn === col.key}
				style="width: {col.width}px; {col.align === 'right' ? 'text-align: right; justify-content: flex-end;' : ''}"
				onclick={() => handleSort(col.key)}
			>
				<span>{col.label}</span>
				{#if sortColumn === col.key}
					<span class="sort-arrow">{sortAsc ? '▲' : '▼'}</span>
				{/if}
			</button>
		{/each}
		<div class="header-cell header-fill"></div>
	</div>

	<div class="grid-body" bind:this={containerEl} onscroll={handleScroll}>
		<div style="height: {totalHeight}px; position: relative;">
			<div style="transform: translateY({offsetY}px);">
				{#each visibleFiles as file, i (file.id)}
					{@const isSelected = $selectedIds.has(file.id)}
					{@const isOdd = (visibleStart + i) % 2 === 1}
					<button
						class="grid-row"
						class:selected={isSelected}
						class:odd={isOdd}
						style="height: {ROW_HEIGHT}px"
						onclick={(e) => handleRowClick(file, e)}
					>
						<div class="cell check-col">
							<input
								type="checkbox"
								class="row-check"
								checked={isSelected}
								onclick={(e) => e.stopPropagation()}
								onchange={() => toggleSelection(file.id, true)}
							/>
						</div>
						{#each columns as col}
							{@const val = getCellValue(file, col.key)}
							<div
								class="cell"
								class:mono={col.mono}
								class:tag-cell={col.tag}
								style="width: {col.width}px; {col.align === 'right' ? 'text-align: right; justify-content: flex-end;' : ''}"
								title={val}
							>
								{val}
							</div>
						{/each}
						<div class="cell cell-fill"></div>
					</button>
				{/each}
			</div>
		</div>
	</div>
</div>

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
</style>
