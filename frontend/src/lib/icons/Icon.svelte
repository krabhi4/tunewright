<script lang="ts">
	interface Props {
		path: string; // inner SVG markup (paths/shapes), viewBox 0 0 24 24
		size?: number;
		stroke?: number;
		label?: string;
	}
	let { path, size = 16, stroke = 1.7, label }: Props = $props();
</script>

<svg
	width={size}
	height={size}
	viewBox="0 0 24 24"
	fill="none"
	stroke="currentColor"
	stroke-width={stroke}
	stroke-linecap="round"
	stroke-linejoin="round"
	role={label ? 'img' : undefined}
	aria-label={label}
	aria-hidden={label ? undefined : 'true'}
>
	{@html path}
</svg>
