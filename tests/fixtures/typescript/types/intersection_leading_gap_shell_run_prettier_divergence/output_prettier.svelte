<script lang="ts">
	// the leading-`&` gap and the first member's stripped shell are contiguous in source,
	// so their comments are ONE run, emitted in source order
	type A =
		/* c1 */
		// d1
		a;

	// the same gap on a 2-member intersection written with the leading `&`
	type B =
		/* c2 */
		// d2
		b & c;

	// an extra redundant layer on the shell changes nothing
	type C =
		/* c3 */
		// d3
		d;

	// a run of blocks in the gap, each keeping its own line
	type E =
		/* c4a */
		/* c4b */
		// d4
		e & f;

	// under an array suffix, where the position's own pair would otherwise claim the shell
	type G =
		/* c5 */
		// d5
		g[];

	// a line comment in the gap takes the same run, ahead of the shell's
	type H =
		// c6
		// d6
		h;

	// the shell one link DEEPER, under an array suffix on a 2-member intersection: the
	// leading gap is the intersection's own, and the member's leading-EDGE shell rides it
	type I =
		/* c7 */
		// d7
		i[] & j;

	// and the same deeper shell reached through the LINE-comment route, which owns the gap
	// through its own emitter - one shape, both routes
	type J =
		// c8
		// d8
		k[] & l;
</script>
