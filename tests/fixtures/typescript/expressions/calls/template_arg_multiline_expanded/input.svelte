<script lang="ts">
	// Multiline template literal as single call arg — expanded form.
	// Prettier preserves this when the author places the template on its own line.

	// Method chain (common test pattern)
	expect(original).toBe(
		`first line
second line
third line
`
	);

	// Member expression callee
	console.log(
		`line 1
line 2
line 3
`
	);

	// Simple callee
	fn(
		`short
content
`
	);

	// A comment leading the template rides the expanded form with it.
	expect(a).toBe(
		/* c1 */
		`with leading
comment
`
	);

	// So does one trailing it, before the `)`.
	expect(b).toBe(
		`with trailing
comment
` /* c2 */
	);

	// Both gaps at once, on a plain member callee.
	obj.method(
		/* c3 */
		`both
gaps
` /* c4 */
	);
</script>
