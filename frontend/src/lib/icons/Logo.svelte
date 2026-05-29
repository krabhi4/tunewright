<script lang="ts">
	let { size = 16 }: { size?: number } = $props();
</script>

<svg width={size} height={size} viewBox="0 0 24 24" fill="none" aria-hidden="true">
	<path
		d="M11 3H19a2 2 0 0 1 2 2v8l-9 9a2 2 0 0 1-2.8 0L3 16a2 2 0 0 1 0-2.8z"
		stroke="currentColor" stroke-width="2" stroke-linejoin="round"
	/>
	<circle cx="16" cy="8" r="2.2" fill="currentColor" />
	<path d="M8.5 13.5l3 3" stroke="currentColor" stroke-width="2" stroke-linecap="round" />
</svg>
