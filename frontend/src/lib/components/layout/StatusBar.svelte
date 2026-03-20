<script lang="ts">
	import { formatTotalDuration, formatSize } from '$lib/utils/format';

	interface Props {
		fileCount: number;
		selectedCount: number;
		totalDuration: number;
		totalSize: number;
		modifiedCount?: number;
	}

	let { fileCount, selectedCount, totalDuration, totalSize, modifiedCount = 0 }: Props = $props();
</script>

<div class="statusbar">
	<span class="stat">{fileCount} file{fileCount !== 1 ? 's' : ''}</span>

	{#if selectedCount > 0}
		<span class="sep">&middot;</span>
		<span class="stat">{selectedCount} selected</span>
	{/if}

	{#if totalDuration > 0}
		<span class="sep">&middot;</span>
		<span class="stat mono">{formatTotalDuration(totalDuration)}</span>
	{/if}

	<span class="sep">&middot;</span>
	<span class="stat mono">{formatSize(totalSize)}</span>

	{#if modifiedCount > 0}
		<span class="sep">&middot;</span>
		<span class="stat modified">{modifiedCount} modified</span>
	{/if}
</div>

<style>
	.statusbar {
		height: var(--statusbar-height);
		background: var(--bg-surface);
		border-top: 1px solid var(--border);
		display: flex;
		align-items: center;
		padding: 0 10px;
		gap: 0;
		flex-shrink: 0;
		user-select: none;
	}

	.stat {
		font-size: 11px;
		color: var(--text-secondary);
	}

	.sep {
		margin: 0 6px;
		color: var(--text-muted);
		font-size: 10px;
	}

	.mono {
		font-family: var(--font-mono);
		font-size: 10.5px;
	}

	.modified {
		color: var(--modified);
	}

	@media (max-width: 768px) {
		.statusbar {
			overflow-x: auto;
			white-space: nowrap;
		}
	}
</style>
