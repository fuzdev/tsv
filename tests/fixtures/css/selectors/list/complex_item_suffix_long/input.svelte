<style>
	/* a non-last item with descendant combinators: 100 columns with the comma - one line */
	.aaaaaaaaaaaaaaaaaaaa .bbbbbbbbbbbbbbbbbbbb .cccccccccccccccccccc .dddddddddddddddddddddddddddddd,
	.x {
		color: red;
	}

	/* the same at 101 columns - breaks at each combinator */
	.aaaaaaaaaaaaaaaaaaaa
		.bbbbbbbbbbbbbbbbbbbb
		.cccccccccccccccccccc
		.ddddddddddddddddddddddddddddddd,
	.x {
		color: red;
	}

	/* a non-last item with child combinators: 100 columns with the comma - one line */
	.aaaaaaaaaaaaaaaaaaaa > .bbbbbbbbbbbbbbbbbbbb > .cccccccccccccccccccc > .dddddddddddddddddddddddd,
	.x {
		color: red;
	}

	/* the same at 101 columns - breaks at each combinator */
	.aaaaaaaaaaaaaaaaaaaa
		> .bbbbbbbbbbbbbbbbbbbb
		> .cccccccccccccccccccc
		> .ddddddddddddddddddddddddd,
	.x {
		color: red;
	}

	/* a non-last :where() item: 100 columns with the comma - :where() stays inline */
	:where(.class1, .class2, .class3, .aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa),
	.class4 {
		color: red;
	}

	/* the same at 101 columns - :where() wraps */
	:where(
		.class1,
		.class2,
		.class3,
		.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
	),
	.class4 {
		color: red;
	}

	/* a last item with descendant combinators: 100 columns with the ` {` - one line */
	.x,
	.aaaaaaaaaaaaaaaaaaaa .bbbbbbbbbbbbbbbbbbbb .cccccccccccccccccccc .ddddddddddddddddddddddddddddd {
		color: red;
	}

	/* the same at 101 columns - breaks at each combinator */
	.x,
	.aaaaaaaaaaaaaaaaaaaa
		.bbbbbbbbbbbbbbbbbbbb
		.cccccccccccccccccccc
		.dddddddddddddddddddddddddddddd {
		color: red;
	}
</style>
