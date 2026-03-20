import { writable } from 'svelte/store';
import { persisted } from './persisted';

export const sidebarWidth = persisted('ts:sidebarWidth', 272);
export const sidebarCollapsed = persisted('ts:sidebarCollapsed', false);
export const filterText = writable('');
export const filterVisible = persisted('ts:filterVisible', false);

// Sort state — synced to URL by +page.svelte
export const sortColumn = writable<string | null>(null);
export const sortAsc = writable(true);

// Increment to force cover art refresh across components
export const coverArtVersion = writable(0);
export function bumpCoverArt() {
	coverArtVersion.update((n) => n + 1);
}
