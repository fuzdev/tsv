<style>
	/* :global() followed by pseudo-class */
	:global(a):hover {
		color: red;
	}

	/* :global() with :has() */
	:global(body):has(.class) {
		background: blue;
	}

	/* :global() with pseudo-element */
	:global(a)::before {
		content: '';
	}

	/* Local class adjacent to :global() with pseudo */
	.class:global(.class1):focus {
		outline: none;
	}

	/* Compound with multiple pseudo-classes */
	:global(a.class):hover:focus {
		opacity: 0.8;
	}
</style>
