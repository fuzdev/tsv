<script>
	// A bare parameter gains parens, and the comment leading it stays ahead of
	// them — printed once, never repeated inside the parens.
	const a = /* c */ (x) => x + 1;

	// The same arrow reached as a call argument and as an array element
	fn(/* c */ (x) => x + 1);
	const b = [/* c */ (x) => x + 1];

	// A curried chain: every head keeps its own comment
	const c = /* c1 */ (x) => /* c2 */ (y) => x + y;

	// A preserved multi-line block keeps the value on the `=` line
	const d = /* c1
c2 */ (x) => x + 1;

	// An indentable one hangs the value onto its own line
	const e =
		/**
		 * c
		 */ (x) => x + 1;
</script>
