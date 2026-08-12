<script lang="ts">
	// A block comment between the binding and `=` stays where it was written.
	for (let a /* c */ = 0; a < 10; a++) {}

	// A later declarator's gap answers the same way.
	for (let b = 0, c /* c */ = 1; b < c; b++) {}

	// An annotated binding keeps the comment after the annotation.
	for (let d: number /* c */ = 0; d < 10; d++) {}

	// A run stays in order, each comment distinct.
	for (let e /* c1 */ /* c2 */ = 0; e < 10; e++) {}

	// A multiline block keeps its place and breaks the header around it.
	for (
		let f /* x
	y */ = 0;
		f < 10;
		f++
	) {}
</script>
