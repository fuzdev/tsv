<style>
	div {
		/* 100 chars - stays inline (at boundary) */
		clip-path: polygon(0% 0%, 10% 0%, 20% 0%, 30% 0%, 40% 0%, 50% 0%, 60% 0%, 70% 0%, 80% 0%, 900%);

		/* 101 chars - wraps */
		clip-path: polygon(
			0% 0%,
			10% 0%,
			20% 0%,
			30% 0%,
			40% 0%,
			50% 0%,
			60% 0%,
			70% 0%,
			80% 0%,
			9000%
		);
	}
</style>
