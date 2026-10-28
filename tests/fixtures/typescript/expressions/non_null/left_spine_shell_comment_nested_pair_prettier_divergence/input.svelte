<script lang="ts">
	// A block comment glued inside a discarded grouping pair below a non-null, standalone or a
	// link of the chain above it, leads the whole value, ahead of every pair the spine prints.
	x = /*c*/ (a.b()! + 1).c;
	x = /*c*/ (a.b()!() + 1).c;
	// a run of commented `!`s, and a `!` link with calls above it
	x = /* c1 */ (a.b /* t1 */! /* t2 */! + 1).c;
	x = (
		a /* c1 */[x]
			?.r!.u() + 1
	).b;
	// the same under a prefix unary, inside the pair the comment earns there
	x = typeof (/*c*/ a.b.c!());
	x = !(/*c*/ a.b.c!());
	// a line comment takes a line of its own ahead of the pair
	// c
	(a.f()! as T)();
	// c
	(a.f()!() as T)();
</script>
