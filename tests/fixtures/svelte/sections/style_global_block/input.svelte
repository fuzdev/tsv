<style>
	:global {
		div {
			color: red;
		}
		p {
			margin: 0;
		}
	}

	.local :global {
		.nested {
			font-size: 1rem;
		}
	}
</style>
