<style>
	div {
		/* a newline inside line names breaks the row there; the track stays beside the close */
		grid-template-columns:
			[a
			b] 1fr;
		grid-template-rows:
			[a] 1fr [b
			c] 2fr;
		/* off the grid properties the line names join */
		grid-auto-rows: [a b] 1fr;
	}
</style>
