<script lang="ts">
	// trailing comment in a parenthesized union type argument
	type A1 = Foo<a | b /* c */>;

	// leading comment in a parenthesized union type argument
	type A2 = Foo</* c */ a | b>;

	// parenthesized union among multiple type arguments
	type A3 = Map<a | b /* c */, c>;
</script>
