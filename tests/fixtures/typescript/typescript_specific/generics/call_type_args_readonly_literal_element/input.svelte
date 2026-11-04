<script lang="ts">
	// `readonly` over an array of a LITERAL element type, as a type argument. An empty
	// `[]` is no member access, so the operand reads only as a type
	fn<readonly 1[]>(x);
	fn<readonly 'a'[]>(x);
	fn<readonly `a`[]>(x);
	fn<readonly `a${B}`[]>(x);
	fn<readonly {}[]>(x);
	fn<readonly { a: 1 }[]>(x);
	fn<readonly 1n[]>(x);

	// An index ahead of the array suffix, and a second suffix
	fn<readonly 1[0][]>(x);
	fn<readonly 'a'[][]>(x);
	fn<readonly 'a' /* c */[]>(x);

	// Every carrier of a type-argument list, and a sibling argument on either side
	new Cls<readonly 1[]>(x);
	const a1 = fn<readonly 1[]>;
	fn<readonly 1[]>`t`;
	fn<readonly 'a'[], B>(x);
	fn<B, readonly 'a'[]>(x);
	fn<readonly 'a'[] | B>(x);
	outer(fn<readonly 'a'[]>(x));
</script>

<p>{fn<readonly 'a'[]>(x)}</p>
