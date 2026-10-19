<style>
	/* a comment-bearing list: 100 columns with the ` {` - one line (prettier agrees) */
	.x, /* c */ .aaaaaaaaaaaa .bbbbbbbbbbbb .cccccccccccc .ddddddddddddddddddddddddddddddddddddddddd {
		color: red;
	}

	/* the same at 101 columns - tsv breaks at each combinator, prettier keeps any authored form */
	.x, /* c */ .aaaaaaaaaaaa .bbbbbbbbbbbb .cccccccccccc .dddddddddddddddddddddddddddddddddddddddddd {
		color: red;
	}

	/* only the last line carries the ` {`: a 99-column head line stays whole, :not() breaks */
	.b, /* c */ :where(.class1, .class2, .class3, .class000000000000000000000000000000000000000):not([data-attr1], [data-attr2]) {
		color: red;
	}

	/* the same with a 100-column head line */
	.b, /* c */ :where(.class1, .class2, .class3, .class0000000000000000000000000000000000000000):not([data-attr1], [data-attr2]) {
		color: red;
	}
</style>
