<script lang="ts">
	// A run the author BROKE AFTER, ahead of a HUG-eligible terminal body of a curried arrow
	// chain — an object, an array or a sequence. The chain's terminal hug arm is the default
	// arrow layout's own, so the run takes prettier's break-after-operator form exactly where
	// a single arrow's would: the run own-line under `=>`, the body below it.

	// an object the author blanked, so the body's own doc carries the break
	const a1 = (x1) => (y1) =>
		/* c */
		({
			aa: 1,

			bb: 2
		});

	// an array that breaks on width
	const a2 = (x2) => (y2) =>
		/* c */
		[1111111111, 2222222222, 3333333333, 4444444444, 5555555555, 6666666666, 77777777];

	// a sequence, with the break forced by a multi-line block in the run
	const a3 = (x3) => (y3) =>
		/**
		 * c
		 */
		(aa, bb);

	// three heads ask the same question of an object
	const a4 = (x4) => (y4) => (z4) =>
		/* c */
		({
			aa: 1,

			bb: 2
		});

	// the chain layout's other context: a call argument
	fn1(aa, (x5) => (y5) =>
		/* c */
		({
			bb: 1,

			cc: 2
		})
	);

	// null control: a body that FITS keeps the glued hug, whose flat render is the same bytes
	const a6 = (x6) => (y6) => /* c */ ({ aa: 1 });

	// null control: no comment
	const a7 = (x7) => (y7) => ({ aa: 1 });
</script>
