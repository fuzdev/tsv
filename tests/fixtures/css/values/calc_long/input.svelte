<style>
	div {
		/* calc() - 100 chars exactly - stays inline (at boundary) */
		min-width: calc(100% - 20px + 10px - 5px + 2px - 1px + 0.5px - 0.25px + 0.125px + 0.00390625px);

		/* calc() - 101 chars - wraps (exceeds print_width) */
		min-width: calc(
			100% - 20px + 10px - 5px + 2px - 1px + 0.5px - 0.25px + 0.125px + 0.001953125px
		);

		/* calc() - expression exceeds 100 chars - wraps with continuation */
		min-width: calc(
			100% - 20px + 10px - 5px + 2px - 1px + 0.5px - 0.25px + 0.125px + 0.001953125px +
				0.0009765625px
		);
	}
</style>
