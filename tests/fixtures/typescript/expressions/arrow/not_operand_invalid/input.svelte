<script lang="ts">
	// An arrow function is an AssignmentExpression, so it stands where one does — an
	// assignment's right side, a conditional's branches, a sequence, an argument, an
	// element — and is NO operand: parenthesized it is a primary any operator takes,
	// bare it is a syntax error behind every unary and binary one (see input_invalid_*).
	const a = x + ((b) => c);
	const d = !((b) => c);
	const e = x ? (b) => c : (b) => d;
	const f = (x, (b) => c);
	const g = [(b) => c, async (b) => c];
	fn(
		(b) => c,
		async () => {}
	);
</script>
