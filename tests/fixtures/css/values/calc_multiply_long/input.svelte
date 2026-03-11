<style>
	div {
		/* multiply with parens - 100 chars exactly - stays inline */
		width: max(
			28rem,
			calc(
				0.5 * (100vw - var(--aaaaaaaaaa) - var(--bbbbbbbbbb) - var(--cccccccccc) - var(--ddddddddd))
			)
		);

		/* multiply with parens - 101 chars - breaks after * */
		width: max(
			28rem,
			calc(
				0.5 *
					(100vw - var(--aaaaaaaaaa) - var(--bbbbbbbbbb) - var(--cccccccccc) - var(--dddddddddd))
			)
		);

		/* inner line 100 chars after * break - stays inline in parens */
		width: max(
			28rem,
			calc(
				0.5 *
					(100vw - var(--aaaaaaaaaa) - var(--bbbbbbbbbb) - var(--cccccccccc) - var(--ddddddddddddd))
			)
		);

		/* inner line 101 chars after * break - inner parens also break */
		width: max(
			28rem,
			calc(
				0.5 *
					(
						100vw - var(--aaaaaaaaaa) - var(--bbbbbbbbbb) - var(--cccccccccc) -
							var(--dddddddddddddd)
					)
			)
		);
	}
</style>
