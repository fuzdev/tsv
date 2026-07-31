<script lang="ts">
	// Trailing line comments on the members of a parenthesized union element: the
	// parens expand and each comment stays with its member.
	type A = (
		| 'a' // c1
		| 'b' // c2
	)[]; // c3

	// The intersection sibling of the same shape.
	type D = (
		| 'a' // c1
		| 'b' // c2
	) &
		E; // c3
</script>
