<script lang="ts">
	import { goto } from '$app/navigation';
	import { logout } from '$lib/api/auth';
	import { auth } from '$lib/stores/auth';

	interface Props {
		onManageUsers?: () => void;
	}

	let { onManageUsers }: Props = $props();
	let open = $state(false);

	let user = $derived($auth.user);
	let isSuperAdmin = $derived(user?.role === 'super_admin');

	async function handleLogout() {
		open = false;
		try {
			await logout();
		} catch {
			// Clear client state even if request fails
		}
		auth.set({ checked: true, setupRequired: false, authenticated: false, user: null });
		goto('/login');
	}

	function handleManageUsers() {
		open = false;
		onManageUsers?.();
	}

	function handleClickOutside(e: MouseEvent) {
		const target = e.target as HTMLElement;
		if (!target.closest('.user-menu')) {
			open = false;
		}
	}
</script>

<svelte:window onclick={handleClickOutside} />

{#if user}
	<div class="user-menu">
		<button class="user-trigger" onclick={() => (open = !open)}>
			<span class="user-avatar">{user.username[0].toUpperCase()}</span>
			<span class="user-name">{user.username}</span>
		</button>

		{#if open}
			<div class="user-dropdown">
				<div class="dropdown-header">
					<span class="dropdown-username">{user.username}</span>
					<span class="dropdown-role">{isSuperAdmin ? 'Super Admin' : 'Admin'}</span>
				</div>
				<div class="dropdown-divider"></div>
				{#if isSuperAdmin}
					<button class="dropdown-item" onclick={handleManageUsers}>
						Manage Users
					</button>
				{/if}
				<button class="dropdown-item dropdown-item--danger" onclick={handleLogout}>
					Sign Out
				</button>
			</div>
		{/if}
	</div>
{/if}

<style>
	.user-menu {
		position: relative;
	}

	.user-trigger {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 3px 8px;
		background: transparent;
		border: none;
		color: var(--text-secondary);
		font-family: var(--font-ui);
		font-size: 12px;
		cursor: pointer;
		border-radius: var(--radius-sm);
		transition: background 0.1s;
	}

	.user-trigger:hover {
		background: var(--bg-hover);
		color: var(--text-primary);
	}

	.user-avatar {
		width: 22px;
		height: 22px;
		border-radius: 50%;
		background: var(--accent-muted);
		color: var(--accent);
		display: flex;
		align-items: center;
		justify-content: center;
		font-size: 11px;
		font-weight: 600;
	}

	.user-name {
		max-width: 100px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.user-dropdown {
		position: absolute;
		top: calc(100% + 4px);
		right: 0;
		min-width: 160px;
		background: var(--bg-surface);
		border: 1px solid var(--border);
		border-radius: var(--radius-md);
		padding: 4px;
		z-index: 100;
		box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
	}

	.dropdown-header {
		padding: 8px 10px;
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.dropdown-username {
		font-size: 12px;
		font-weight: 500;
		color: var(--text-primary);
	}

	.dropdown-role {
		font-size: 11px;
		color: var(--text-muted);
	}

	.dropdown-divider {
		height: 1px;
		background: var(--border);
		margin: 2px 0;
	}

	.dropdown-item {
		display: block;
		width: 100%;
		padding: 6px 10px;
		background: none;
		border: none;
		color: var(--text-secondary);
		font-family: var(--font-ui);
		font-size: 12px;
		text-align: left;
		cursor: pointer;
		border-radius: var(--radius-sm);
	}

	.dropdown-item:hover {
		background: var(--bg-hover);
		color: var(--text-primary);
	}

	.dropdown-item--danger:hover {
		color: var(--error);
	}
</style>
