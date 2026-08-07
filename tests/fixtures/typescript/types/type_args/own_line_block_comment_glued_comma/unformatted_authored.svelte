<script lang="ts">
	// An own-line block comment with the comma glued after leads the next type argument
	type A = Base<T
/* c */, U>;

	// Written before the comma instead, it trails the previous type argument
	type B = Base<T /* c */, U>;

	// A line comment expands the list - the comment still leads its argument, on its line
	type C = Base<T
/* c */, U // t
>;
</script>
