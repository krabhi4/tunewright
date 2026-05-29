export const ICONS = {
	open: '<path d="M3 6h6l2 2h10v11H3z"/><path d="M3 6v13"/>',
	save: '<path d="M5 4h11l3 3v13H5z"/><path d="M8 4v5h7V4"/><path d="M8 20v-6h8v6"/>',
	fnToTag: '<path d="M4 7h16"/><path d="M4 12h11"/><path d="M4 17h7"/><path d="M16 12l4 5-4 5" transform="translate(0 -5)"/>',
	rename: '<path d="M4 12h12"/><path d="M13 7l5 5-5 5"/><path d="M20 5v14"/>',
	actions: '<path d="M5 6h14"/><path d="M5 12h9"/><path d="M5 18h5"/><path d="M16 14l4 4-4 4" transform="translate(0 -4)"/>',
	lookup: '<circle cx="11" cy="11" r="6"/><path d="M20 20l-4-4"/>',
	users: '<circle cx="9" cy="8" r="3"/><path d="M3 20a6 6 0 0 1 12 0"/><path d="M16 5a3 3 0 0 1 0 6"/><path d="M21 20a6 6 0 0 0-4-5.6"/>',
	sun: '<circle cx="12" cy="12" r="4.5"/><path d="M12 2v2M12 20v2M4 12H2M22 12h-2M5 5l1.5 1.5M17.5 17.5L19 19M19 5l-1.5 1.5M6.5 17.5L5 19"/>',
	moon: '<path d="M20 13a8 8 0 1 1-9-9 6.5 6.5 0 0 0 9 9z"/>',
	folder: '<path d="M3 6h6l2 2h10v11H3z"/>',
	command: '<path d="M9 6a3 3 0 1 0-3 3h12a3 3 0 1 0-3-3v12a3 3 0 1 0 3-3H6a3 3 0 1 0 3 3z"/>'
} as const;

export type IconName = keyof typeof ICONS;
