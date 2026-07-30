<style>
	.c {
		/* the fallback is a comma token, not an empty fallback */
		color: var(--a,,);
		/* an escaped comma is content, so this fallback is the single ident `x,` */
		background: var(--b,x\,);
	}
</style>
