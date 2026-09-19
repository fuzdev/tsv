<script lang="ts">
	// a type assertion whose asserted type opens with a generic function type's `<` keeps a
	// space after its own `<`: tsc never splits a `<<` token at an assertion
	const a1 = <
		<T>() => R
	>x;
	const a2 = <
		<T>(v: T) => T>(<U>x);
	const a3 = <
		<T>() => R | S>x;

	// a comment at the asserted type's end changes nothing about what follows the `<`
	const a4 = <
		<T>() => R /* c */>x;

	// a comment ahead of the type already separates the two `<`
	const a5 = </* c */
		<T>() => R>x;

	// no second `<` follows the first: a constructor type, a plain function type
	const a6 = <
		new <T>() => R>x;
	const a7 = <
		() => R>x;
</script>

{<
	<T>() => R>x}
