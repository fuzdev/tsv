<script lang="ts">
	// A block comment after the comma leads the next type parameter - it stays there
	function fn1<T, /* c */ U>() {}

	// Classes take the same rule
	class Multiple<T, /* c */ U> {}

	// Contrast - a comment before the comma trails the previous parameter
	function fn2<T /* c */, U>() {}

	// A line comment expands the list - the glued comment still leads its parameter
	function fn3<
		T,
		/* c1 */ U, // c2
		V
	>() {}
</script>
