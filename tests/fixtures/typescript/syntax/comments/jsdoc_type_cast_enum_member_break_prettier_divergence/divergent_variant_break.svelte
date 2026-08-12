<script lang="ts">
	// An enum member's `=` is a value gap: the cast keeps its comment glued to the `(` and
	// expands inside the preserved parens, the layout every wide cast takes.
	enum E1 {
		A = /** @type {A} */
		b1,
		B = /** @type {A} */
		someObject.somePropertyName.someOtherProperty.andAnotherOne.andYetMore.andFinallyOne,
		// A comment the author gave a line of its own hangs the value instead, the `=` ending
		// its line — the one shape whose break is the comment's rather than the width's.
		C = /** @type {A} */
		someObject.somePropertyName.someOtherProperty.andAnotherOne.andYetMore.andFinal
	}
</script>
