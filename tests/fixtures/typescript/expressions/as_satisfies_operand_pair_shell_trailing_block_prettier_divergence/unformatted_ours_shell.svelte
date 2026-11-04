<script lang="ts">
	// A block comment ending a redundant pair inside a cast operand that takes the pair the
	// cast needs around a binary operand trails that pair, ahead of the keyword.
	x = (a + (b + c /* c1 */)) as T;

	const y = (a - (b /* c2 */)) satisfies T;

	(a + (b /* c3 */)) as T;

	function f1() {
		return (a * (b /* c4 */)) as T;
	}

	f(g((a + (b /* c5 */)) as T));

	o = { k: (a ?? (b /* c6 */)) as T };

	// The pair written around the whole operand: the comment already trails it.
	z = (a + b /* c7 */) as number;

	// A statement header's test.
	if ((a - (b /* c9 */)) as T) {
	}
</script>

<p>{(a + (b /* c8 */)) as T}</p>
