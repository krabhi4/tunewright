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
		busy?: boolean;
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
		onExtra,
		busy = false
	}: Props = $props();
</script>

<Modal {title} {open} onClose={() => { if (!busy) onCancel(); }}>
	<p class="confirm-message">{message}</p>
	<div class="confirm-actions">
		<button class="btn btn-secondary" onclick={onCancel} disabled={busy}>{cancelLabel}</button>
		{#if extraLabel && onExtra}
			<button class="btn btn-danger" onclick={onExtra} disabled={busy}>{extraLabel}</button>
		{/if}
		<button class="btn btn-primary" onclick={onConfirm} disabled={busy}>{confirmLabel}</button>
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
