<script lang="ts">
	// `<<` splits into type arguments in a class-extends heritage clause —
	// prettier's typescript parser cannot split it here (see prettier_rejects.txt)
	const b1 = class extends fn<<T>(v: T) => void> {};
</script>
