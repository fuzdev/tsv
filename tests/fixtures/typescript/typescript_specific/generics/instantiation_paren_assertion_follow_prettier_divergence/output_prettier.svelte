<script lang="ts">
	// A pair around an instantiation expression stays ahead of `as` and `satisfies`. To
	// acorn-typescript a word after the closing `>` continues a comparison, so bare, the
	// line either does not parse (`f<T> as T`) or reads as a different program
	// (`f<T> as [T]` is `f < T > as[T]`); tsc reads both as the assertion.
	f<T> as T;
	f<T> satisfies T;
	x = a.b<T> as T;
	x = f()<T> satisfies T;
	x = f<A<B>> as const;
	x = a?.b<T> as T;
	g(f<T> as T, f<T> satisfies T);
	x = f<T> as A as B;
	x = c ? (f<T> as T) : d;

	// The pair wraps whatever operand ENDS on the close: a prefix operator's argument
	// re-lexes the same way, and a pair written around the instantiation alone moves out
	// to it (a binary operand of `as` takes its pair in both formatters already).
	x = -f<T> as T;
	x = typeof f<T> satisfies T;
	x = (a * f<T>) as T;

	// An operand that does not end on the close prints bare in both formatters.
	x = f as T;
	x = f<T>(a) satisfies T;
</script>

{f<T> as T}
{#if a.b<T> satisfies T}a{/if}
<div data-a={f<T> as T}></div>
{#each f<T> as T[] as item}{item}{/each}
