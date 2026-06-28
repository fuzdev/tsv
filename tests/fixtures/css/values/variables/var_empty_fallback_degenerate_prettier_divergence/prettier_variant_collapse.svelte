<style>
	.c {
		/* A value-less var() fallback collapses to the canonical empty-fallback form */
		color: var(--a, ,);
	}
</style>
