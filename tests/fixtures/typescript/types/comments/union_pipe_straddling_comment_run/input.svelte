<script lang="ts">
	// an own-line comment before the `|` and one trailing the `|` both lead the member,
	// each on its own line, with no blank between them
	let w:
		| A
		// x
		// c
		| B;

	// an authored blank line before the run is preserved
	let v:
		| A

		// x
		// c
		| B;

	// and a blank between the two comments is preserved
	let u:
		| A
		// x

		// c
		| B;
</script>
