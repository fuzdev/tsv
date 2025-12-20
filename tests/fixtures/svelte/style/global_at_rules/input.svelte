<style>
	/* :global() inside @media */
	@media (min-width: 768px) {
		:global(.class) {
			display: block;
		}
	}

	/* :global() inside @supports */
	@supports (display: grid) {
		:global(.class) {
			display: grid;
		}
	}

	/* :global() inside @layer */
	@layer base {
		:global(.class) {
			margin: 0;
		}
	}

	/* :global() inside nested @media */
	@media screen {
		@media (min-width: 768px) {
			:global(.class) {
				padding: 0;
			}
		}
	}
</style>
