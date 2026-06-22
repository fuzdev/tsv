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
</script>
