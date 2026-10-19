<style>
	/* head (the line through `:not(`) at 98 columns - :where() stays inline, :not() wraps */
	:where(.class1, .class2, .class3, .aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa):not(
		[data-attr1],
		[data-attr2]
	) {
		color: red;
	}
	/* head at 100 columns - the last argument list breaks: :where() stays inline, :not() wraps */
	:where(.class1, .class2, .class3, .aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa):not(
		[data-attr1],
		[data-attr2]
	) {
		color: red;
	}
	/* head at 101 columns - :where() wraps, :not() stays inline */
	:where(
		.class1,
		.class2,
		.class3,
		.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
	):not([data-attr1], [data-attr2]) {
		color: red;
	}
	/* after a descendant combinator: continuation-line head at 100 columns - :not() wraps */
	.class4
		:where(.class1, .class2, .class3, .aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa):not(
			[data-attr1],
			[data-attr2]
		) {
		color: red;
	}
	/* after a descendant combinator: continuation-line head at 101 columns - :where() wraps */
	.class4
		:where(
			.class1,
			.class2,
			.class3,
			.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
		):not([data-attr1], [data-attr2]) {
		color: red;
	}
	@media (min-width: 1px) {
		/* in an at-rule: head at 100 columns - :not() wraps */
		:where(.class1, .class2, .class3, .aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa):not(
			[data-attr1],
			[data-attr2]
		) {
			color: red;
		}
		/* in an at-rule: head at 101 columns - :where() wraps */
		:where(
			.class1,
			.class2,
			.class3,
			.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
		):not([data-attr1], [data-attr2]) {
			color: red;
		}
	}
	/* inside :global(): head at 100 columns - :not() wraps */
	:global(
		:where(.class1, .class2, .class3, .aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa):not(
			[data-attr1],
			[data-attr2]
		)
	) {
		color: red;
	}
	/* inside :global(): head at 101 columns - :where() wraps */
	:global(
		:where(
			.class1,
			.class2,
			.class3,
			.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
		):not([data-attr1], [data-attr2])
	) {
		color: red;
	}
	/* selector list: head at 100 columns - :not() wraps before the comma */
	:where(.class1, .class2, .class3, .aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa):not(
		[data-attr1],
		[data-attr2]
	),
	.class4 {
		color: red;
	}
	/* selector list: head at 101 columns - :where() wraps */
	:where(
		.class1,
		.class2,
		.class3,
		.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
	):not([data-attr1], [data-attr2]),
	.class4 {
		color: red;
	}
</style>
