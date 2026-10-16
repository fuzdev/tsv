<script lang="ts">
	// A second type argument list after a first list whose indexed access a `|` or `&`
	// continues is no list: tsc takes none ahead of a `<`, so `f<A[K] | B><C>(x)` is the
	// comparison `(f < A[K]) | (B > <C>(x))`, the type assertion `<C>(x)` on the right of its
	// `>`. The pairs are the spelling every parser reads as that comparison.
	(f < A[K]) | (B > <C>x);
	(f < A[K]) & (B > <C>x);
	(f < A[0]) | (B > <C>x);
	(new f() < A[K]) | (B > <C>x);

	// The same second list after a first list that reads as a type argument list on its own.
	(f < A) | (B > <C>x);
	(f < A['k']) | (B > <C>x);
	(f < B) | (A[K] > <C>x);
	(new f() < A) | (B > <C>x);
</script>

{(f < A[K]) | (B > <C>x)}
{(f < A) | (B > <C>x)}
