<script lang="ts">
	// A block glued through a sealed base's leading gap rides the operand's line inside
	// the base's expanding parens when the chain breaks — the comment-free base's
	// layout, the block leading the operand.
	(
		/* c1 */ (a as B) // m
			?.q
	).m;
	(
		/* c2 */ x // m
			?.x
	)();
	(
		/* c3 */ (a as B)
			?.m()
			.m()
			.c(() => d)
	).z;

	// A kept `)!` seal and a restricted production's argument take the same layout.
	(
		/* c5 */ (a as B) // m
			?.q
	)!.m;
	function g() {
		throw (
			/* c6 */ (a as B)
				?.m()
				.m()
				.c(() => d)
		).z;
	}

	// A block trailing a breaking chain stays beside its last link, inside the parens.
	(
		a
			?.m()
			.m()
			.c(() => d) /* c7 */
	).z;

	// When the chain fits, the block leads it flat.
	(/* c4 */ (a as B)?.q).m;
</script>
