<script lang="ts" context="module">
	// Named export
	export abstract class Single {
		abstract fn(): void;
	}

	// Default export (must have name in Svelte)
	export default abstract class Base {
		abstract fn(): void;
	}
</script>
