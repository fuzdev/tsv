<script lang="ts">
	// An own-line block comment with the comma glued after leads the next element
	type A = [a, /* c */ b];

	// Written before the comma instead, it trails the previous element
	type B = [a /* c */, b];

	// A line comment expands the tuple - the comment still leads its element, on its line
	type C = [a

/* c */, b // t
];
</script>
