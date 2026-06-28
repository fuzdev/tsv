<style>
	div {
		/* Function comma-item at 100 - stays inline within the multiline list */
		background:
			linear-gradient(to right, rgba(0, 0, 0, 0) 0%, rgba(255, 255, 255, 1) 100000000000000000000%),
			top left;
		/* Function comma-item at 101 - wraps to stay within 100 */
		background:
			linear-gradient(
				to right,
				rgba(0, 0, 0, 0) 0%,
				rgba(255, 255, 255, 1) 1000000000000000000000%
			),
			top left;
		/* Final function item at 101 - trailing ; reserved, also wraps */
		background:
			top left,
			linear-gradient(
				to right,
				rgba(0, 0, 0, 0) 0%,
				rgba(255, 255, 255, 1) 1000000000000000000000%
			);
	}
</style>
