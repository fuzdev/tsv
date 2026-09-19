<script lang="ts">
	// The TERMINAL `=>`→body gap of a curried arrow chain is answered by the same emitters
	// the default arrow layout's `=>`→body arms use, so a run the author BROKE AFTER keeps
	// its break wherever that machinery keeps it — across every body kind, and whether the
	// break is forced by a multi-line block in the run or by the body's own hard break.

	// a body whose own doc breaks forces the run's break: a multi-line template
	const a1 = (x1) => (y1) =>
		/* c */
		`aa
bb`;

	// an object literal the author blanked, which breaks for the same reason
	const a2 = (x2) => (y2) =>
		/* c */
		({
			aa: 1,

			bb: 2
		});

	// a call whose function argument breaks it
	const a3 = (x3) => (y3) =>
		/* c */
		ff(bb, (zz) => {
			return zz;
		});

	// a multi-line block in the run forces the break with a body that fits: a member chain
	const a4 = (x4) => (y4) =>
		/**
		 * c
		 */
		/* d */ b4.c4().d4();

	// the same, with a binary body, and with the heads a destructured parameter force-breaks
	const a5 =
		({ x5 }) =>
		({ y5 }) =>
			/**
			 * c
			 */
			/* d */ b5 + c5 + d5;

	// a logical body, three heads deep
	const a7 = (x7) => (y7) => (z7) =>
		/**
		 * c
		 */
		/* d */ b7 && c7 && d7;

	// the chain layout's own host: a call argument, whose heads a destructured parameter
	// force-breaks — the one shape that reaches this gap through the flattened layout with
	// its heads stacked
	fn1(
		aa,
		({ x8 }) =>
			({ y8 }) =>
				/**
				 * c
				 */
				/* d */ b8.c8().d8()
	);

	// null control: no break to keep — the run glues to the body
	const a6 = (x6) => (y6) => /* c */ b6.c6().d6();
</script>
