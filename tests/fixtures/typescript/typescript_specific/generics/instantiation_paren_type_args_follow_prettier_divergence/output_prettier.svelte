<script lang="ts">
	// A paren pair around an instantiation expression stays when a second type argument
	// list follows its close, whatever that list belongs to: a `new`, a call, a tagged
	// template, or another instantiation. Bare, the `<` opening the second list is a token
	// tsc's grammar never takes a first list ahead of, so `f<T><U>(x)` is the comparison
	// `f < T > <U>(x)`, with a type assertion on its right.
	new f<T><U>(x);
	new a.b<T><U>();
	new f<T><U>`x`();
	f<T><U>(x);
	f<T><U>`x`;
	const a = f<T><U>;

	// In a member chain the call prints its own list and the base prints the first.
	f<T><U>(x).y;
	f<T /* a */><U /* b */>(x).y();
	f<T><U><V>(x).y;
	f<T><U>(x)!;

	// A class heritage: bare, tsc reads `extends f<T>` and stops at the second `<`.
	class D extends f<T><U> {}
	const E = class extends a.b<T><U> {};
</script>

<!-- The same pair in a template expression. -->
{new f<T><U>(x)}
{f<T><U>(x)}
{f<T><U>}
