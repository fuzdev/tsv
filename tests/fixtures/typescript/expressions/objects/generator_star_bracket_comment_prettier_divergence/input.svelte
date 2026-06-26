<script lang="ts">
	// Comment between the generator `*` and `[` is preserved after the star;
	// prettier relocates it inside the brackets before the key (see output_prettier.svelte).
	const o = {
		*/* c1 */ [a]() {},
		async */* c2 */ [b]() {}
	};
</script>
