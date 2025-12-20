<style>
	/* :is() - matches-any pseudo-class */
	.class1:is(.active, .focused) {
		color: red;
	}

	div:is(#id1, #id2, .class1) {
		background: blue;
	}

	/* :where() - zero-specificity variant */
	:where(h1, h2, h3) {
		margin: 10px;
	}

	article :where(h1, h2) {
		padding: 5px;
	}

	/* :not() - negation pseudo-class */
	input:not(.disabled, [readonly]) {
		border: 1px solid green;
	}

	div:not(#id1):not(.class1) {
		opacity: 0.5;
	}
</style>
