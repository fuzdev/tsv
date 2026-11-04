<script lang="ts">
	// A comment written last inside a redundant pair down a sequence's last operand, at a
	// value position, stays inside the pair the sequence prints there.
	x = (a, b < c /* c1 */);

	const y = (a, (b, c /* c2 */));

	f = () => (a, b < c /* c3 */);

	let z = (a, b < c /* c4 */),
		w = 1;

	g = () => (
		a,
		b <
			c // c5
	);
</script>
