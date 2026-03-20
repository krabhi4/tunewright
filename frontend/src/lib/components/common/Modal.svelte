<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		title: string;
		open: boolean;
		onClose: () => void;
		children: Snippet;
	}

	let { title, open, onClose, children }: Props = $props();

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') onClose();
	}

	function handleBackdrop(e: MouseEvent) {
		if (e.target === e.currentTarget) onClose();
	}
</script>

{#if open}
	<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
	<div class="backdrop" role="dialog" aria-modal="true" tabindex="-1" onclick={handleBackdrop} onkeydown={handleKeydown}>
		<div class="modal">
			<div class="modal-header">
				<span class="modal-title">{title}</span>
				<button class="modal-close" onclick={onClose}>&times;</button>
			</div>
			<div class="modal-body">
				{@render children()}
			</div>
		</div>
	</div>
{/if}

<style>
	.backdrop {
		position: fixed;
		inset: 0;
		background: rgba(0, 0, 0, 0.6);
		display: flex;
		align-items: center;
		justify-content: center;
		z-index: 100;
	}

	.modal {
		background: var(--bg-elevated);
		border: 1px solid var(--border);
		border-radius: var(--radius-md);
		min-width: 400px;
		max-width: 600px;
		max-height: 70vh;
		display: flex;
		flex-direction: column;
		box-shadow: 0 8px 32px rgba(0, 0, 0, 0.4);
	}

	.modal-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 10px 14px;
		border-bottom: 1px solid var(--border);
		flex-shrink: 0;
	}

	.modal-title {
		font-size: 13px;
		font-weight: 600;
		color: var(--text-primary);
	}

	.modal-close {
		background: none;
		border: none;
		color: var(--text-muted);
		font-size: 18px;
		cursor: pointer;
		padding: 0 4px;
		line-height: 1;
	}

	.modal-close:hover {
		color: var(--text-primary);
	}

	.modal-body {
		padding: 14px;
		overflow-y: auto;
	}
</style>
