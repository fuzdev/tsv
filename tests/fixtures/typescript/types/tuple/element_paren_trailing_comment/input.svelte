<script lang="ts">
	// A shell's trailing line comment strips with the shell (last element)
	type A = [
		string,
		number // c
	];

	// Middle element: the comment trails past the comma
	type B = [
		string,
		number, // c
		boolean
	];

	// A trailing block comment strips with the shell too
	type C = [string, number /* c */, boolean];

	// A leading own-line block and a trailing line comment in one shell
	type D = [
		string,
		/* lead */
		number // c
	];

	// An own-line trailing block defers past the comma, leading the next element
	type E = [
		string,
		number,
		/* c */
		boolean
	];

	// The same on the last element stays before the `]`
	type F = [
		string,
		number
		/* c */
	];
</script>
