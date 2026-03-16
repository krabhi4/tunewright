<script lang="ts">
	import { filterText, filterVisible } from '$lib/stores/ui';

	let inputEl: HTMLInputElement;

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			filterText.set('');
			filterVisible.set(false);
		}
	}

	$effect(() => {
		if ($filterVisible && inputEl) {
			inputEl.focus();
		}
	});
</script>

{#if $filterVisible}
	<div class="filterbar">
		<span class="filter-icon">&#128269;</span>
		<input
			bind:this={inputEl}
			type="text"
			placeholder="Filter files..."
			bind:value={$filterText}
			onkeydown={handleKeydown}
			class="filter-input"
		/>
		{#if $filterText}
			<button class="filter-clear" onclick={() => filterText.set('')}>&times;</button>
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
		font-size: 12px;
		color: var(--text-muted);
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
