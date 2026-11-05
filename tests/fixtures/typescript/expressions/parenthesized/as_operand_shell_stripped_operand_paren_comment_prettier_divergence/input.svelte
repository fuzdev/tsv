<script lang="ts">
	// a comment written last inside a redundant pair down the right edge of an `as` or
	// `satisfies` operand stays inside the operand's pair, ahead of a comment written after the
	// pair, each on a line of its own
	k1 = (
		a + b // c1
		// d1
	) as T;
	k2 = (
		a + (b + c) // c2
		// d2
	) as T;
	k3 = (
		a + b
		// c3
		// d3
	) as T;
	k4 = (
		a + b // c4
		// d4
	) satisfies T;
	fn1(
		(
			a + b // c5
			// d5
		) as T
	);

	// an own-line comment after the pair already takes a line of its own, and the non-null
	// operand keeps the two comments apart
	k6 = (
		a + b // c6
		// d6
	) as T;
	k7 = (
		a + b // c7
		// d7
	)!;

	// with nothing written after the pair, the comment trails the statement
	k8 = (a + b) as T; // c8
	k11 = (a + b)!; // c11

	// a block written ahead of the comment after the pair leaves the two in the same order, the
	// non-null operand's pair included
	k9 = (
		a + b // c9
		/* x9 */ // d9
	) as T;
	k10 = (
		a + b // c10
		/* x10 */ // d10
	)!;

	// a block alone after the pair stays after the comment, in the order written
	k12 = (
		a + b // c12
		/* x12 */
	)!;

	// a non-null operand a member or a call follows keeps the same order
	k13 = (
		a + b // c13
		/* x13 */ // d13
	)!.m;
	k14 = (
		a + b // c14
		/* x14 */ // d14
	)!();
	k15 = (
		a + (b + c)
		// c15
		// d15
	)!.m;
	k16 = (
		a + b // c16
		/* x16 */
	)!.m;

	// with nothing written after the pair, the comment trails the statement ahead of a member too
	k17 = (a + b)!.m; // c17
</script>
