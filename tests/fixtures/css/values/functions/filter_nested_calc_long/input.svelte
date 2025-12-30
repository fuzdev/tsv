<style>
	div {
		/* drop-shadow with nested calc() - 100 chars exactly - stays inline (at boundary) */
		filter: drop-shadow(calc(10px + 5px) calc(20px - 10px) calc(5px * 2) rgba(0, 0, 0, 0.55555555));

		/* drop-shadow with nested calc() - 101 chars - wraps (exceeds print_width) */
		filter: drop-shadow(
			calc(10px + 5px) calc(20px - 10px) calc(5px * 2) rgba(0, 0, 0, 0.555555555)
		);

		/* drop-shadow with nested calc() - inner line exceeds 100 chars - wraps with continuation */
		filter: drop-shadow(
			calc(100px + 50px + 25px + 12px) calc(200px - 100px - 50px - 25px) calc(50px * 2 + 10px + 5px)
				rgba(0, 0, 0, 0.5)
		);
	}
</style>
