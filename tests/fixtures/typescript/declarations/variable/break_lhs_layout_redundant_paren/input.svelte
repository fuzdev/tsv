<script lang="ts">
	// A complex type annotation — a reference with 2+ type arguments, one of them itself generic
	// or a conditional — takes the break-lhs layout: the arguments break, the initializer stays
	// on the `=` line. A redundant paren shell around the argument, or around the whole
	// annotation (stripped on output), must not change that verdict.
	const aaaaaaaaaaaaaaaaaaaaaaaaa: Map<
		string,
		A extends B ? C : D
	> = someVeryLongIdentifierThatOverflowsTheLinexx;
	const bbbbbbbbbbbbbbbbbbbbbbbbb: Map<
		string,
		Array<number>
	> = someVeryLongIdentifierThatOverflowsTheLinexxxxxxx;
	const ccccccccccccccccccccccccc: Map<
		string,
		Array<number>
	> = someVeryLongIdentifierThatOverflowsTheLinexxxxxxx;
</script>
