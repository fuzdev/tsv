<style>
	:global(body) {
		margin: 0;
	}

	div :global(span) {
		color: red;
	}

	.local :global(.external) {
		font-size: 1rem;
	}
</style>
