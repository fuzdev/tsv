<script lang="ts">
	// Arrow parameter defaults dividing an operand that ends in `}` — the scan for the `)`
	// that closes the parameter list reads the `/` as the division it is, not as a regex
	// opener that runs on to a later `/` on the same line.

	// After an object literal
	const a = (x = {} / 2) => x / 2;
	const b = (x = { y: 1 } / 2) => x / 2;

	// An async head
	const c = async (x = {} / 2) => x / 2;

	// Arrow arguments
	fn((x = {} / (2 / 1)) => x);
	fn((x = {} / 2) => x, y / 2);

	// A conditional's consequent arrow with a return type
	const d = cond ? (x = {} / 2): T => x / 2 : y;

	// A slash later on the line, in a comment
	const e = (x = {} / 2) => x; // a/b

	// The division inside a template interpolation
	const f = (x = `${{} / 2}`) => x / 2;
</script>

{(x = {} / 2) => x}
