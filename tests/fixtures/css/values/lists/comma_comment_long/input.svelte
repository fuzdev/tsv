<style>
	div {
		/* leading run before single-node elements: 100 columns stays inline */
		font-family: /* c */ f0000000, f1111111, f2222222, f3333333, f4444444, aaaaaaaaaaaaaaaaaaaaaaaa;
		/* 101 columns: the run stays on the colon's line and the list breaks beneath it */
		font-family: /* c */
			f0000000, f1111111, f2222222, f3333333, f4444444, aaaaaaaaaaaaaaaaaaaaaaaaa;
		/* a comment inside a function element: 100 columns stays inline */
		transition: a, fn(/* c */ x), f2222222, f3333333, f4444444, f5555555, aaaaaaaaaaaaaaaaaaaaaaaaa;
		/* 101 columns breaks after the colon like the comment-free list */
		transition:
			a, fn(/* c */ x), f2222222, f3333333, f4444444, f5555555, aaaaaaaaaaaaaaaaaaaaaaaaaa;
		/* a long list fills greedily beneath the run */
		font-family: /* c */
			f0000000, f1111111, f2222222, f3333333, f4444444, f5555555, f6666666, f7777777, f8888888,
			f9999999, faaaaaaa;
		/* a custom property's comma list never breaks per element; a comment in a space-separated */
		/* element rides the greedy fill, 101 columns */
		--prop:
			a /* c */ b, c d, e f, g h, aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa;
	}
</style>
