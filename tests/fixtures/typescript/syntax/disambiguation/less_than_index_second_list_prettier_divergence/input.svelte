<script lang="ts">
	// A second type argument list after a first list whose indexed access a `|` or `&`
	// continues. To acorn-typescript, whose tree Svelte compiles, `f<A[K] | B><C>(x)` calls
	// the instantiation `f<A[K] | B>` with the type arguments `<C>`; to tsc, which takes no
	// list ahead of a `<`, the same text is the comparison `f < A[K] | B > <C>(x)`. Printed
	// as written, each parser reads the output the way it read the input.
	f<A[K] | B><C>(x);
	f<A[K] & B><C>(x);
	f<A[0] | B><C>(x);
	new f<A[K] | B><C>(x);

	// The same second list after a first list that is a type argument list on its own.
	f<A | B><C>(x);
	f<A['k'] | B><C>(x);
	f<B | A[K]><C>(x);
	new f<A | B><C>(x);
</script>

{f<A[K] | B><C>(x)}
{f<A | B><C>(x)}
