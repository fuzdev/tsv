<style>
	/* :global() with :root */
	:global(:root) {
		--color: red;
	}

	/* :global() with universal selector */
	:global(*) {
		box-sizing: border-box;
	}

	/* :global() with html and body */
	:global(html),
	:global(body) {
		margin: 0;
	}

	/* :global() with attribute selector */
	:global([data-attr]) {
		display: block;
	}

	/* :global() with :not() */
	:global(:not(.class)) {
		opacity: 1;
	}
</style>
