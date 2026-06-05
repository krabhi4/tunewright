<script lang="ts">
	import { fly } from 'svelte/transition';
	import { toasts, dismissToast, type ToastKind } from '$lib/stores/toast';

	const labels: Record<ToastKind, string> = {
		success: 'OK',
		error: 'Error',
		warning: 'Warning',
		info: 'Info'
	};

	// Per-kind accent token; adding a kind without an entry here is a TS error.
	const accents: Record<ToastKind, string> = {
		success: 'var(--success)',
		error: 'var(--error)',
		warning: 'var(--warning)',
		info: 'var(--state-info)'
	};
</script>

<!--
	No aria-live on the container: each toast carries its own live-region role
	(status = polite, alert = assertive). A wrapping polite region would
	override the assertive announcement of error toasts.
-->
<div class="toast-stack">
	{#each $toasts as t (t.id)}
		<div
			class="toast"
			style:--toast-accent={accents[t.kind]}
			role={t.kind === 'error' ? 'alert' : 'status'}
			transition:fly={{ y: 8, duration: 150 }}
		>
			<span class="toast-label">{labels[t.kind]}</span>
			<span class="toast-message">{t.message}</span>
			<button
				class="toast-dismiss"
				onclick={() => dismissToast(t.id)}
				aria-label="Dismiss notification"
			>
				&times;
			</button>
		</div>
	{/each}
</div>

<style>
	.toast-stack {
		position: fixed;
		right: 12px;
		bottom: calc(var(--statusbar-height) + 12px);
		z-index: var(--z-toast);
		display: flex;
		flex-direction: column;
		gap: 8px;
		width: min(380px, calc(100vw - 24px));
		pointer-events: none;
	}

	.toast {
		display: flex;
		align-items: baseline;
		gap: 8px;
		padding: 9px 10px;
		background: var(--bg-elevated);
		border: 1px solid var(--border);
		border-left: 3px solid var(--toast-accent);
		border-radius: var(--radius-md);
		box-shadow: var(--shadow-dropdown);
		font-family: var(--font-ui);
		pointer-events: auto;
	}

	.toast-label {
		flex-shrink: 0;
		font-size: 10px;
		font-weight: 600;
		letter-spacing: 0.06em;
		text-transform: uppercase;
		color: var(--toast-accent);
	}

	.toast-message {
		flex: 1;
		font-size: 12px;
		line-height: 1.45;
		color: var(--text-primary);
		overflow-wrap: anywhere;
	}

	.toast-dismiss {
		flex-shrink: 0;
		align-self: flex-start;
		display: flex;
		align-items: center;
		justify-content: center;
		/* 24px hit target (WCAG 2.5.8) without inflating the visual row */
		min-width: 24px;
		min-height: 24px;
		margin: -6px -6px -6px 0;
		background: none;
		border: none;
		padding: 0;
		font-size: 14px;
		line-height: 1;
		color: var(--text-muted);
		cursor: pointer;
	}

	.toast-dismiss:hover {
		color: var(--text-primary);
	}
</style>
