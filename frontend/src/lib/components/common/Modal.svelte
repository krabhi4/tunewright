<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		title: string;
		open: boolean;
		onClose: () => void;
		children: Snippet;
		wide?: boolean;
	}

	let { title, open, onClose, children, wide = false }: Props = $props();

	let backdropEl = $state<HTMLDivElement>();

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') onClose();
		if (e.key === 'Tab' && backdropEl) {
			trapFocus(e);
		}
	}

	function trapFocus(e: KeyboardEvent) {
		if (!backdropEl) return;
		const focusable = backdropEl.querySelectorAll<HTMLElement>(
			'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
		);
		if (focusable.length === 0) return;
		const first = focusable[0];
		const last = focusable[focusable.length - 1];

		if (e.shiftKey) {
			if (document.activeElement === first) {
				e.preventDefault();
				last.focus();
			}
		} else {
			if (document.activeElement === last) {
				e.preventDefault();
				first.focus();
			}
		}
	}

	function handleBackdrop(e: MouseEvent) {
		if (e.target === e.currentTarget) onClose();
	}

	$effect(() => {
		if (open && backdropEl) {
			const focusable = backdropEl.querySelector<HTMLElement>(
				'input:not([type="hidden"]), button:not(:disabled), select, textarea, [tabindex]:not([tabindex="-1"])'
			);
			if (focusable) {
				focusable.focus();
			} else {
				backdropEl.focus();
			}
		}
	});
</script>

{#if open}
	<div
		class="backdrop"
		role="dialog"
		aria-modal="true"
		aria-labelledby="modal-title"
		tabindex="-1"
		bind:this={backdropEl}
		onclick={handleBackdrop}
		onkeydown={handleKeydown}
	>
		<div class="modal" class:modal-wide={wide}>
			<div class="modal-header">
				<h2 class="modal-title" id="modal-title">{title}</h2>
				<button class="modal-close" onclick={onClose} aria-label="Close dialog">&times;</button>
			</div>
			<div class="modal-body">
				{@render children()}
			</div>
		</div>
	</div>
{/if}

<style>
	@keyframes backdrop-in {
		from { opacity: 0; }
		to { opacity: 1; }
	}

	@keyframes modal-in {
		from { opacity: 0; transform: scale(0.97); }
		to { opacity: 1; transform: scale(1); }
	}

	.backdrop {
		position: fixed;
		inset: 0;
		background: var(--backdrop);
		display: flex;
		align-items: center;
		justify-content: center;
		z-index: var(--z-modal);
		outline: none;
		animation: backdrop-in 150ms cubic-bezier(0.25, 1, 0.5, 1);
	}

	.modal {
		background: var(--bg-elevated);
		border: 1px solid var(--border);
		border-radius: var(--radius-md);
		min-width: min(400px, calc(100vw - 32px));
		max-width: calc(100vw - 32px);
		max-height: min(650px, calc(100vh - 32px));
		width: min(600px, calc(100vw - 32px));
		display: flex;
		flex-direction: column;
		box-shadow: var(--shadow-modal);
		animation: modal-in 150ms cubic-bezier(0.25, 1, 0.5, 1);
		will-change: opacity, transform;
		resize: both;
		overflow: hidden;
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
		margin: 0;
	}

	.modal-close {
		background: none;
		border: none;
		color: var(--text-muted);
		font-size: 18px;
		cursor: pointer;
		padding: 4px 8px;
		line-height: 1;
		min-width: 32px;
		min-height: 32px;
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.modal-close:hover {
		color: var(--text-primary);
	}

	.modal-body {
		padding: 14px;
		overflow-y: auto;
		flex: 1;
		min-height: 0;
	}

	.modal-wide {
		width: min(850px, calc(100vw - 32px));
		max-height: min(800px, calc(100vh - 32px));
	}
</style>
