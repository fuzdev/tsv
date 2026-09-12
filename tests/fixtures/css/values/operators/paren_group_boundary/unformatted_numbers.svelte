<style>
	div {
		/* two adjacent groups are two members of one value, so they take the
		ordinary member gap whether or not the author wrote one */
		margin: (1.50) (2.50);
		padding: (1.50) (2.50);
		/* an operator glued AFTER a group keeps its glue */
		height: (1.50)/2.50;
		/* `*` is spaced beside a group as it is anywhere else */
		width: 1.50 * (2.50);
		/* the `font` carve-out keeps a glued `/` before a group, so prettier's own
		output holds the glue this shape otherwise loses */
		font: 1.50/(2.50);
	}
</style>
