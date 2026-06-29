<script lang="ts">
	// An inline block hugging the first argument stays on the argument's line in a
	// member chain. An own-line comment preceding the hug keeps its own line — it
	// is NOT collapsed onto the hug's line.

	// own-line block comment, then a hugging block
	obj
		.method(
			/* o */
			/* b */ a
		)
		.other();

	// own-line line comment, then a hugging block
	obj
		.method(
			// c
			/* b */ d
		)
		.other();
</script>
