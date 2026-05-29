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
	<span class="stat"><span class="mono">{fileCount}</span> file{fileCount !== 1 ? 's' : ''}</span>

	{#if selectedCount > 0}
		<span class="sep">&middot;</span>
		<span class="stat"><span class="mono">{selectedCount}</span> selected</span>
	{/if}

	{#if totalDuration > 0}
		<span class="sep">&middot;</span>
		<span class="stat mono">{formatTotalDuration(totalDuration)}</span>
	{/if}

	<span class="sep">&middot;</span>
	<span class="stat mono">{formatSize(totalSize)}</span>

	{#if modifiedCount > 0}
		<span class="sep">&middot;</span>
		<span class="stat edited"><span class="mono">{modifiedCount}</span> edited</span>
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
		font-family: var(--font-ui);
	}

	.stat {
		font-size: 11px;
		color: var(--text-secondary);
	}

	.sep {
		margin: 0 8px;
		color: var(--text-muted);
		font-size: 10px;
	}

	.mono {
		font-family: var(--font-mono);
		font-feature-settings: "tnum" 1;
		font-size: 10.5px;
	}

	.edited {
		color: var(--state-dirty);
	}

	@media (max-width: 768px) {
		.statusbar {
			overflow-x: auto;
			white-space: nowrap;
		}
	}
</style>
