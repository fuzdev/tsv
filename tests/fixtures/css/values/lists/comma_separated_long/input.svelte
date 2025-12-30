<style>
	div {
		/* 100 chars - stays inline (at boundary) */
		font-family: 'font00000000000000000', 'font1111111111111', 'font22222222222', 'font33333333333';
		/* 101 chars - value wraps to next line */
		font-family:
			'font00000000000000000', 'font11111111111111', 'font22222222222', 'font33333333333';
		/* Long font-family list - exceeds 100 chars per line, tests comma-separated wrapping */
		font-family:
			'f000000000', 'f111111111', 'f222222222', 'f333333333', 'f444444444', 'f555555555',
			'f666666666', 'f777777777', 'f888888888', 'f999999999';
		/* Multiple long gradients - exceeds 100 chars, tests outer comma list + function arg wrapping */
		background:
			linear-gradient(
				0deg,
				rgba(0, 0, 0, 0) 0%,
				rgba(1, 1, 1, 0.11111111) 50%,
				rgba(2, 2, 2, 0.22222222) 100%
			),
			radial-gradient(circle at center, #000 0%, #111 50%, #222 100%);
	}
</style>
