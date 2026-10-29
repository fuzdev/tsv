<script lang="ts">
	// a `//` leading the index from inside a stripped shell prints at the index's
	// leading edge, and the lookup lays out as for the comment written there
	x[(// c1
	y).p]();

	// the optional lookup, and a member call after it
	x?.[(
		// c2
		y).p].m();

	// a value, a `return`, and a member-only chain
	q = x[(// c3
	y).p]();

	function f() {
		return x[(// c4
		y).p]();
	}

	q = a.b[(// c5
	y).p].c;

	// an index nested in an index
	x[y[(// c6
	z).p]]();

	// the broken index still breaks the rest of the chain, and a lookup after a call
	// starts a group of its own, as prettier groups them
	x[(// c8
	y).p].m().n();

	a.b()[(// c9
	y).p].m().n();

	// a blank inside the brackets is the lookup's, not a blank between methods, and a
	// `//` inside a trailing lookup's brackets keeps the lookup in the chain's tail
	a.b()[

	(y)].m().n();

	q = a.b().c() /* c10 */[x][(// c11
	y).p];
</script>

<p>{x[(// c7
y).p]()}</p>
