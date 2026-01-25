<script lang="ts">
	// Simple mapped type
	const a = fn<{
		[K in keyof T]: V;
	}>();

	// Mapped type with modifiers
	const b = fn<{
		readonly [K in keyof T]?: V;
	}>();

	// Mapped type with method
	const c = obj.method<{
		[K in keyof T]: V;
	}>('arg');
</script>
