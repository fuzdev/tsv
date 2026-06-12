<style>
	/* :global() inside CSS nesting */
	div {
		:global(.class) {
			color: red;
		}
	}

	/* Nested with & combinator */
	.class {
		& :global(a) {
			text-decoration: none;
		}
	}

	/* :global() as ancestor with & */
	.class1 {
		:global(.class2) & {
			background: blue;
		}
	}

	/* Deeply nested */
	.class1 {
		.class2 {
			:global(.class3) {
				margin: 0;
			}
		}
	}
</style>
