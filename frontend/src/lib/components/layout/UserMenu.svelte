<script lang="ts">
	import { goto } from '$app/navigation';
	import { logout } from '$lib/api/auth';
	import { auth } from '$lib/stores/auth';

	interface Props {
		onManageUsers?: () => void;
	}

	let { onManageUsers }: Props = $props();
	let open = $state(false);
	let dropdownEl = $state<HTMLDivElement>();

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

	function toggleOpen() {
		open = !open;
	}

	$effect(() => {
		if (open && dropdownEl) {
			const firstItem = dropdownEl.querySelector<HTMLElement>('[role="menuitem"]');
			firstItem?.focus();
		}
	});

	function handleDropdownKeydown(e: KeyboardEvent) {
		if (!dropdownEl) return;
		const items = Array.from(dropdownEl.querySelectorAll<HTMLElement>('[role="menuitem"]'));
		const currentIdx = items.indexOf(document.activeElement as HTMLElement);

		switch (e.key) {
			case 'ArrowDown':
				e.preventDefault();
				items[(currentIdx + 1) % items.length]?.focus();
				break;
			case 'ArrowUp':
				e.preventDefault();
				items[(currentIdx - 1 + items.length) % items.length]?.focus();
				break;
			case 'Escape':
				e.preventDefault();
				open = false;
				break;
		}
	}
</script>

<svelte:window onclick={handleClickOutside} />

{#if user}
	<div class="user-menu">
		<button
			class="user-trigger"
			onclick={toggleOpen}
			aria-haspopup="true"
			aria-expanded={open}
		>
			<span class="user-avatar">{user.username[0].toUpperCase()}</span>
			<span class="user-name">{user.username}</span>
		</button>

		{#if open}
			<!-- svelte-ignore a11y_interactive_supports_focus -->
			<div
				class="user-dropdown"
				role="menu"
				bind:this={dropdownEl}
				onkeydown={handleDropdownKeydown}
			>
				<div class="dropdown-header">
					<span class="dropdown-username">{user.username}</span>
					<span class="dropdown-role">{isSuperAdmin ? 'Super Admin' : 'Admin'}</span>
				</div>
				<div class="dropdown-divider" role="separator"></div>
				{#if isSuperAdmin}
					<button class="dropdown-item" role="menuitem" onclick={handleManageUsers}>
						Manage Users
					</button>
				{/if}
				<button class="dropdown-item dropdown-item--danger" role="menuitem" onclick={handleLogout}>
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

	@keyframes dropdown-in {
		from { opacity: 0; transform: translateY(-4px); }
		to { opacity: 1; transform: translateY(0); }
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
		z-index: var(--z-dropdown);
		box-shadow: var(--shadow-dropdown);
		animation: dropdown-in 120ms cubic-bezier(0.25, 1, 0.5, 1);
		will-change: opacity, transform;
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

	.dropdown-item:hover,
	.dropdown-item:focus-visible {
		background: var(--bg-hover);
		color: var(--text-primary);
		outline: none;
	}

	.dropdown-item--danger:hover,
	.dropdown-item--danger:focus-visible {
		color: var(--error);
	}
</style>
