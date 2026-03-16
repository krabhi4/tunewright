import { writable } from 'svelte/store';

export const sidebarWidth = writable(272);
export const sidebarCollapsed = writable(false);
export const filterText = writable('');
export const filterVisible = writable(false);
