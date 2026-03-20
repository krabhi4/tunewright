<script lang="ts">
	import Modal from './Modal.svelte';

	interface Props {
		open: boolean;
		title: string;
		message: string;
		confirmLabel?: string;
		cancelLabel?: string;
		extraLabel?: string;
		onConfirm: () => void;
		onCancel: () => void;
		onExtra?: () => void;
	}

	let {
		open,
		title,
		message,
		confirmLabel = 'Confirm',
		cancelLabel = 'Cancel',
		extraLabel,
		onConfirm,
		onCancel,
		onExtra
	}: Props = $props();
</script>

<Modal {title} {open} onClose={onCancel}>
	<p class="confirm-message">{message}</p>
	<div class="confirm-actions">
		<button class="btn btn-secondary" onclick={onCancel}>{cancelLabel}</button>
		{#if extraLabel && onExtra}
			<button class="btn btn-warning" onclick={onExtra}>{extraLabel}</button>
		{/if}
		<button class="btn btn-primary" onclick={onConfirm}>{confirmLabel}</button>
	</div>
</Modal>

<style>
	.confirm-message {
		font-size: 12.5px;
		color: var(--text-secondary);
		margin: 0 0 16px;
		line-height: 1.5;
	}

	.confirm-actions {
		display: flex;
		justify-content: flex-end;
		gap: 8px;
	}

	.btn {
		padding: 5px 14px;
		border-radius: var(--radius-sm);
		cursor: pointer;
		font-family: var(--font-ui);
		font-size: 12px;
		border: none;
	}

	.btn-secondary {
		background: transparent;
		border: 1px solid var(--border);
		color: var(--text-secondary);
	}

	.btn-secondary:hover {
		background: var(--bg-hover);
	}

	.btn-warning {
		background: var(--error);
		color: white;
	}

	.btn-warning:hover {
		opacity: 0.9;
	}

	.btn-primary {
		background: var(--accent);
		color: white;
	}

	.btn-primary:hover {
		background: var(--accent-hover);
	}
</style>
