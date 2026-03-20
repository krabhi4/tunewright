<script lang="ts">
	import Modal from '$lib/components/common/Modal.svelte';
	import ConfirmModal from '$lib/components/common/ConfirmModal.svelte';
	import {
		listUsers,
		deleteUser,
		createInvite,
		listInvites,
		deleteInvite,
		type UserInfo,
		type InviteInfo
	} from '$lib/api/auth';
	import { auth } from '$lib/stores/auth';

	interface Props {
		open: boolean;
		onClose: () => void;
	}

	let { open, onClose }: Props = $props();

	let users = $state<UserInfo[]>([]);
	let invites = $state<InviteInfo[]>([]);
	let newInviteLink = $state('');
	let error = $state('');
	let loading = $state(false);

	let currentUsername = $derived($auth.user?.username);

	let deleteConfirmOpen = $state(false);
	let pendingDeleteUser = $state<{ id: string; username: string } | null>(null);

	$effect(() => {
		if (open) {
			loadData();
		} else {
			newInviteLink = '';
			error = '';
		}
	});

	async function loadData() {
		loading = true;
		error = '';
		try {
			[users, invites] = await Promise.all([listUsers(), listInvites()]);
		} catch (err: any) {
			error = err.message || 'Failed to load data';
		} finally {
			loading = false;
		}
	}

	async function handleCreateInvite() {
		error = '';
		try {
			const invite = await createInvite();
			newInviteLink = window.location.origin + invite.link;
			invites = await listInvites();
		} catch (err: any) {
			error = err.message || 'Failed to create invite';
		}
	}

	function requestDeleteUser(id: string, username: string) {
		pendingDeleteUser = { id, username };
		deleteConfirmOpen = true;
	}

	async function confirmDeleteUser() {
		if (!pendingDeleteUser) return;
		error = '';
		deleteConfirmOpen = false;
		try {
			await deleteUser(pendingDeleteUser.id);
			users = await listUsers();
		} catch (err: any) {
			error = err.message || 'Failed to delete user';
		}
		pendingDeleteUser = null;
	}

	function cancelDeleteUser() {
		deleteConfirmOpen = false;
		pendingDeleteUser = null;
	}

	async function handleDeleteInvite(token: string) {
		error = '';
		try {
			await deleteInvite(token);
			invites = await listInvites();
		} catch (err: any) {
			error = err.message || 'Failed to delete invite';
		}
	}

	let copied = $state(false);

	async function handleCopyLink() {
		try {
			await navigator.clipboard.writeText(newInviteLink);
			copied = true;
			setTimeout(() => (copied = false), 2000);
		} catch {
			// Clipboard denied — user can manually select from input
		}
	}

	function formatRole(role: string) {
		return role === 'super_admin' ? 'Super Admin' : 'Admin';
	}

	function formatDate(iso: string) {
		return new Date(iso).toLocaleDateString(undefined, {
			month: 'short',
			day: 'numeric',
			year: 'numeric'
		});
	}

	function formatExpiry(iso: string) {
		const diff = new Date(iso).getTime() - Date.now();
		if (diff <= 0) return 'Expired';
		const hours = Math.floor(diff / 3600000);
		const mins = Math.floor((diff % 3600000) / 60000);
		return hours > 0 ? `${hours}h ${mins}m` : `${mins}m`;
	}
</script>

<Modal title="Manage Users" {open} {onClose}>
	{#if loading}
		<div class="um-loading">Loading...</div>
	{:else}
		{#if error}
			<div class="um-error">{error}</div>
		{/if}

		<section class="um-section">
			<h3 class="um-heading">Users</h3>
			<div class="um-table" role="table" aria-label="Users">
				<div class="um-row um-row--header" role="row" aria-hidden="true">
					<div class="um-cell um-cell--name" role="columnheader">Name</div>
					<div class="um-cell um-cell--role" role="columnheader">Role</div>
					<div class="um-cell um-cell--date" role="columnheader">Created</div>
					<div class="um-cell um-cell--action" role="columnheader">Action</div>
				</div>
				{#each users as user}
					<div class="um-row" role="row">
						<div class="um-cell um-cell--name" role="cell">
							{user.username}
							{#if user.username === currentUsername}
								<span class="um-badge um-badge--you">you</span>
							{/if}
						</div>
						<div class="um-cell um-cell--role" role="cell">
							<span
								class="um-badge"
								class:um-badge--super={user.role === 'super_admin'}
							>
								{formatRole(user.role)}
							</span>
						</div>
						<div class="um-cell um-cell--date" role="cell">{formatDate(user.created_at)}</div>
						<div class="um-cell um-cell--action" role="cell">
							{#if user.username !== currentUsername}
								<button
									class="um-btn-delete"
									onclick={() => requestDeleteUser(user.id, user.username)}
								>
									Remove
								</button>
							{/if}
						</div>
					</div>
				{/each}
			</div>
		</section>

		<section class="um-section">
			<div class="um-section-header">
				<h3 class="um-heading">Invites</h3>
				<button class="um-btn-invite" onclick={handleCreateInvite}>Create Invite</button>
			</div>

			{#if newInviteLink}
				<div class="um-invite-link">
					<input type="text" value={newInviteLink} readonly class="um-link-input" aria-label="Invite link" />
					<button class="um-btn-copy" onclick={handleCopyLink}>{copied ? 'Copied' : 'Copy'}</button>
				</div>
			{/if}

			{#if invites.length > 0}
				<div class="um-table" role="table" aria-label="Invites">
					{#each invites as invite}
						<div class="um-row" role="row">
							<div class="um-cell um-cell--token" role="cell" title={invite.token}>
								{invite.token.slice(0, 8)}...
							</div>
							<div class="um-cell um-cell--expiry" role="cell">
								{formatExpiry(invite.expires_at)}
							</div>
							<div class="um-cell um-cell--action" role="cell">
								<button
									class="um-btn-delete"
									onclick={() => handleDeleteInvite(invite.token)}
								>
									Revoke
								</button>
							</div>
						</div>
					{/each}
				</div>
			{:else if !newInviteLink}
				<div class="um-empty">No active invites</div>
			{/if}
		</section>
	{/if}
</Modal>

<ConfirmModal
	open={deleteConfirmOpen}
	title="Remove User"
	message={pendingDeleteUser ? `Remove user "${pendingDeleteUser.username}"? This cannot be undone.` : ''}
	confirmLabel="Remove"
	cancelLabel="Cancel"
	onConfirm={confirmDeleteUser}
	onCancel={cancelDeleteUser}
/>

<style>
	.um-loading,
	.um-empty {
		color: var(--text-muted);
		font-size: 12px;
		text-align: center;
		padding: 16px 0;
	}

	.um-error {
		color: var(--error);
		font-size: 12px;
		text-align: center;
		margin-bottom: 12px;
	}

	.um-section {
		margin-bottom: 18px;
	}

	.um-section:last-child {
		margin-bottom: 0;
	}

	.um-section-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		margin-bottom: 8px;
	}

	.um-heading {
		font-size: 12px;
		font-weight: 600;
		color: var(--text-secondary);
		text-transform: uppercase;
		letter-spacing: 0.5px;
		margin-bottom: 8px;
	}

	.um-section-header .um-heading {
		margin-bottom: 0;
	}

	.um-table {
		display: flex;
		flex-direction: column;
		gap: 1px;
	}

	.um-row--header {
		position: absolute;
		width: 1px;
		height: 1px;
		padding: 0;
		margin: -1px;
		overflow: hidden;
		clip: rect(0, 0, 0, 0);
		white-space: nowrap;
		border: 0;
	}

	.um-row {
		display: flex;
		align-items: center;
		padding: 6px 8px;
		border-radius: var(--radius-sm);
		background: var(--bg-surface);
		gap: 8px;
	}

	.um-row:hover {
		background: var(--bg-hover);
	}

	.um-cell {
		font-size: 12px;
		color: var(--text-secondary);
	}

	.um-cell--name {
		flex: 1;
		color: var(--text-primary);
		display: flex;
		align-items: center;
		gap: 6px;
	}

	.um-cell--role {
		width: 90px;
	}

	.um-cell--date {
		width: 80px;
		font-size: 11px;
		color: var(--text-muted);
	}

	.um-cell--token {
		flex: 1;
		font-family: var(--font-mono);
		font-size: 11px;
	}

	.um-cell--expiry {
		width: 60px;
		font-size: 11px;
		color: var(--text-muted);
	}

	.um-cell--action {
		width: 60px;
		text-align: right;
	}

	.um-badge {
		display: inline-block;
		font-size: 10px;
		padding: 1px 6px;
		border-radius: 3px;
		background: var(--bg-base);
		color: var(--text-muted);
	}

	.um-badge--super {
		background: var(--accent-muted);
		color: var(--accent);
	}

	.um-badge--you {
		background: var(--bg-base);
		color: var(--text-muted);
		font-style: italic;
	}

	.um-btn-delete {
		background: none;
		border: none;
		color: var(--text-muted);
		font-family: var(--font-ui);
		font-size: 11px;
		cursor: pointer;
		padding: 2px 6px;
		border-radius: var(--radius-sm);
	}

	.um-btn-delete:hover {
		color: var(--error);
		background: var(--error-10);
	}

	.um-btn-invite {
		background: var(--accent);
		border: none;
		color: var(--text-on-accent);
		font-family: var(--font-ui);
		font-size: 11px;
		padding: 4px 10px;
		border-radius: var(--radius-sm);
		cursor: pointer;
	}

	.um-btn-invite:hover {
		background: var(--accent-hover);
	}

	.um-invite-link {
		display: flex;
		gap: 6px;
		margin-bottom: 10px;
	}

	.um-link-input {
		flex: 1;
		background: var(--bg-base);
		border: 1px solid var(--border);
		color: var(--text-primary);
		font-family: var(--font-mono);
		font-size: 11px;
		padding: 5px 8px;
		border-radius: var(--radius-sm);
		outline: none;
	}

	.um-link-input:focus {
		border-color: var(--accent);
	}

	.um-btn-copy {
		background: var(--bg-surface);
		border: 1px solid var(--border);
		color: var(--text-secondary);
		font-family: var(--font-ui);
		font-size: 11px;
		padding: 4px 10px;
		border-radius: var(--radius-sm);
		cursor: pointer;
		white-space: nowrap;
	}

	.um-btn-copy:hover {
		background: var(--bg-hover);
		color: var(--text-primary);
	}
</style>
