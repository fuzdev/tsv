<style>
	/* Compound selector inside :global() */
	:global(a.class#id[data-attr]) {
		color: blue;
	}

	/* Multiple :global() with combinators */
	:global(a) > :global(b) + :global(c) ~ :global(d) {
		margin: 0;
	}

	/* Descendant chain inside :global() */
	:global(.class1 .class2 .class3) {
		padding: 0;
	}

	/* Multiple selectors in :global() */
	:global(a, b, c) {
		display: block;
	}
</style>
