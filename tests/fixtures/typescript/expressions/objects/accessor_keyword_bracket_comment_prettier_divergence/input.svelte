<script lang="ts">
	// Comment between the accessor keyword and `[` is preserved after the keyword;
	// prettier relocates it inside the brackets before the key (see output_prettier.svelte).
	const o = {
		get /* c1 */ [a]() {
			return 1;
		},
		set /* c2 */ [b](v) {}
	};
</script>
