<style>
	/* Deeply nested logical combinations */
	:is(:is(.class1, .class2), :where(#id1, #id2)) {
		color: red;
	}

	:not(:is(.class1):not(.class2)) {
		background: blue;
	}

	/* All features combined */
	div:is(.class1, .class2):has(> span):not(.class3) {
		padding: 10px;
	}

	:where(article, section):has(:is(h2, h3)) {
		margin: 20px;
	}
</style>
