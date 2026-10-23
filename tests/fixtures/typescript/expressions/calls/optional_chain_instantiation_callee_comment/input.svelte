<script lang="ts">
	// between an instantiation callee and its type arguments
	T/* c1 */ <U>?.().a;
	T/* c2 */ <U>?.(b).a;
	a.T/* c3 */ <U>?.(b).c;
	x = T/* c4 */ <U>?.(b).a;

	// between the type arguments and the `?.`
	T<U> /* c5 */?.(b).a;
	T<U> /* c6 */?.().a;

	// after the `?.`, leading the first argument
	T<U>?.(/* c7 */ b).a;
</script>

<p>{T/* c8 */ <U>?.(b).a}</p>
