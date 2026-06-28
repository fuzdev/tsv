<style>
	div {
		/* 100 chars - single token fits inline at the boundary */
		prop: fn(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa);
		/* 101 chars - single token wraps onto its own line */
		prop: fn(
			aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
		);
		/* nested 100: outer wraps, inner at boundary stays flat on its own line */
		prop: outer(
			inner(11111111, 11111111, 11111111, 11111111, 11111111, 11111111, 11111111, 11111111, 1111111)
		);
		/* nested 101: outer wraps, inner over boundary also breaks (cascade) */
		prop: outer(
			inner(
				11111111,
				11111111,
				11111111,
				11111111,
				11111111,
				11111111,
				11111111,
				11111111,
				11111111
			)
		);
	}
</style>
