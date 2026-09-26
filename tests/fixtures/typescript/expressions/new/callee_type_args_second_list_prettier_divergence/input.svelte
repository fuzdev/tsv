<script lang="ts">
	// A second type argument list after the list on an argument-less `new` callee. To
	// acorn-typescript, whose tree Svelte compiles, `new f<T><U>(x)` constructs the
	// instantiation `f<T>` with the type arguments `<U>`; to tsc, which takes no list ahead of
	// a `<`, the same text is the comparison `(new f < T) > <U>(x)`. Printed as written, each
	// parser reads the output the way it read the input.
	new f<T><U>(x);
	new a.b<T><U>(x);
	new f<T><U>(x)(y);
	new f<T><U>(x, y);
	// No argument list is added after a tag: to tsc it would join the assertion's operand.
	new f<T><U>`x`;
	new f<A<B>><U>(x);
	const a = new f<T><U>(x);
	g(new f<T><U>(x));

	// The call twin prints as written too.
	f<T><U>(x);
	f<T><U>(x).y;
	f<T /* a */><U /* b */>(x).y();
</script>

{new f<T><U>(x)}
{f<T><U>(x)}
