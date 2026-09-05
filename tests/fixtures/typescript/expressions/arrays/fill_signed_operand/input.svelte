<script>
	/* signed numbers fill like plain numbers */
	const a = [
		-1, -2, -3, -4, -5, -6, -7, -8, -9, -10, -11, -12, -13, -14, -15, -16, -17, -18, -19, -20, -21,
		-22, -23, -24, -25, +26, +27
	];

	/* a comment between the sign and its number disqualifies the fill - one per line */
	const b = [
		-(/* c */ 1),
		-2,
		-3,
		-4,
		-5,
		-6,
		-7,
		-8,
		-9,
		-10,
		-11,
		-12,
		-13,
		-14,
		-15,
		-16,
		-17,
		-18,
		-19,
		-20,
		-21,
		-22,
		-23,
		-24,
		-25,
		+26,
		+27
	];

	/* a doubled sign is not a signed number - one per line */
	const c = [
		-(-1),
		-2,
		-3,
		-4,
		-5,
		-6,
		-7,
		-8,
		-9,
		-10,
		-11,
		-12,
		-13,
		-14,
		-15,
		-16,
		-17,
		-18,
		-19,
		-20,
		-21,
		-22,
		-23,
		-24,
		-25,
		+26,
		+27
	];
</script>
