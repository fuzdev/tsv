<style>
	/* Basic :has() with combinators */
	a:has(> img) {
		display: block;
	}

	li:has(+ li) {
		margin-bottom: 10px;
	}

	/* :has() with multiple selectors */
	article:has(h2, h3) {
		padding: 20px;
	}

	/* :has() with negation */
	div:has(:not(.class1)) {
		background: lightgray;
	}

	/* Chained :has() */
	section:has(> h2):has(> p) {
		border: 1px solid red;
	}

	/* :has() combined with :is() and :not() */
	:is(.class1, .class2):has(span) {
		color: blue;
	}

	div:not(:has(.class1)) {
		opacity: 0.5;
	}
</style>
