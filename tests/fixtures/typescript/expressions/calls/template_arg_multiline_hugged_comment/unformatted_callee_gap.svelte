<script lang="ts">
	// A comment leading the template rides the hugged form with it.

	// Plain call
	fn/* c1 */(`line 1
line 2
`);

	// `new`
	new Comp/* c2 */(`line 1
line 2
`);

	// Member chain
	obj.method/* c3 */(`line 1
line 2
`);

	// Two glued comments
	fn/* c4 */(/* c5 */ `line 1
line 2
`);

	// A multiline block comment still glues the template to the `(` line
	fn(/* c6
c7 */ `line 1
line 2
`);

	// Type arguments
	fn<T>/* c8 */(`line 1
line 2
`);

	// Control: a trailing block stays on the template's last line
	fn(`line 1
line 2
` /* c9 */);

	// Dynamic import
	import/* c10 */(`line 1
line 2
`);

	// Member chain — flat, and the chain heads stay on one line
	a.b().c().d/* c11 */(`line 1
line 2
`);
</script>
