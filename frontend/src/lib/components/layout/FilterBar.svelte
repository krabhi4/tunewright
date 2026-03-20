<script lang="ts">
	import { filterText, filterVisible } from '$lib/stores/ui';

	interface Props {
		matchCount?: number;
		totalCount?: number;
	}

	let { matchCount, totalCount }: Props = $props();

	let inputEl = $state<HTMLInputElement>();
	let localValue = $state($filterText);
	let debounceTimer: ReturnType<typeof setTimeout>;

	function handleInput(e: Event) {
		const val = (e.target as HTMLInputElement).value;
		localValue = val;
		clearTimeout(debounceTimer);
		debounceTimer = setTimeout(() => filterText.set(val), 150);
	}

	function clearFilter() {
		localValue = '';
		clearTimeout(debounceTimer);
		filterText.set('');
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			clearFilter();
			filterVisible.set(false);
		}
	}

	$effect(() => {
		if ($filterVisible && inputEl) {
			localValue = $filterText;
			inputEl.focus();
		}
	});
</script>

{#if $filterVisible}
	<div class="filterbar">
		<svg class="filter-icon" aria-hidden="true" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><circle cx="6.5" cy="6.5" r="4"/><path d="M14 14l-4-4"/></svg>
		<input
			bind:this={inputEl}
			type="text"
			placeholder="Filter files..."
			aria-label="Filter files"
			value={localValue}
			oninput={handleInput}
			onkeydown={handleKeydown}
			class="filter-input"
		/>
		{#if localValue && matchCount != null && totalCount != null}
			<span class="filter-count">{matchCount} of {totalCount}</span>
		{/if}
		{#if localValue}
			<button class="filter-clear" onclick={clearFilter} aria-label="Clear filter">&times;</button>
		{/if}
	</div>
{/if}

<style>
	.filterbar {
		height: 32px;
		background: var(--bg-elevated);
		border-bottom: 1px solid var(--border);
		display: flex;
		align-items: center;
		padding: 0 10px;
		gap: 6px;
		flex-shrink: 0;
	}

	.filter-icon {
		width: 13px;
		height: 13px;
		color: var(--text-muted);
		flex-shrink: 0;
	}

	.filter-input {
		flex: 1;
		background: transparent;
		border: none;
		color: var(--text-primary);
		font-family: var(--font-ui);
		font-size: 12px;
		outline: none;
	}

	.filter-input::placeholder {
		color: var(--text-placeholder);
	}

	.filter-count {
		font-size: 11px;
		font-family: var(--font-mono);
		color: var(--text-muted);
		flex-shrink: 0;
	}

	.filter-clear {
		background: none;
		border: none;
		color: var(--text-muted);
		cursor: pointer;
		font-size: 16px;
		padding: 0 4px;
		line-height: 1;
	}

	.filter-clear:hover {
		color: var(--text-primary);
	}
</style>
