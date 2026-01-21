<script lang="ts">
	// Line comment trailing first variant
	type T =A// trailing
		|B;

	// Line comment trailing multiple variants
	type U =A// trailing A
		|B// trailing B
		|C;

	// Multiple consecutive trailing comments (one per line)
	type V =A// trailing 1
		|B// trailing 2
</script>
