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
</script>
