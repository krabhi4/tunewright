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
			<button class="btn btn-danger" onclick={onExtra}>{extraLabel}</button>
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
</style>
