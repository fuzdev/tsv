<script lang="ts">
	// Line comment between tag and template
	foo
	// c1
	`x`;

	// Multiple line comments
	foo
	// c1
	// c2
	`x`;

	// Block comment inline
	foo /* c1 */ `x`;

	// Multiple block comments inline
	foo /* c1 */ /* c2 */ `x`;

	// Block comment on own line
	foo
	/* c1 */
	`x`;

	// Member expression tag with line comment
	a.b
	// comment
	`x`;

	// Call expression tag with line comment
	fn()
	// comment
	`x`;
</script>
