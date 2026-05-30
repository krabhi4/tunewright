<script lang="ts">
	let { size = 16 }: { size?: number } = $props();
</script>

<!-- Tuning fork: "tune" as a precision instrument (the wright's tool). -->
<svg width={size} height={size} viewBox="0 0 24 24" fill="none" aria-hidden="true">
	<path
		d="M9 4V13a3 3 0 0 0 6 0V4"
		stroke="currentColor"
		stroke-width="2"
		stroke-linecap="round"
		stroke-linejoin="round"
	/>
	<path d="M12 16V20.5" stroke="currentColor" stroke-width="2" stroke-linecap="round" />
	<circle cx="12" cy="21.4" r="1.3" fill="currentColor" />
</svg>
