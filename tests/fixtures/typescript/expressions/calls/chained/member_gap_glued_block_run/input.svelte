<script lang="ts">
	// A comment glued after a multiline block stays in the object's trailing run;
	// the member drops to its own line.
	const a = foo /* m1
	m2 */ /* c6 */
		.b();

	// A `//` glued after the multiline block trails the same way.
	const b = foo /* m1
	m2 */ // c
		.b();

	// Control: a single-line glued run keeps the chain flat.
	const c = foo /* c1 */ /* c2 */
		.b();
</script>
