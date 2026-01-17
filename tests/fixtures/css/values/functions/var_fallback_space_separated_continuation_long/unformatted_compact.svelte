<style>
	.a {
		/* Long comment that forces separate line aaaaaaaaaaaaaaaaaaaaaaaaaaa */
		prop: var(
			--a,
			/* Long comment that forces separate line aaaaaaaaaaaaaaaaaaaaaaaaaaa */
			var(--aaaaaaaaaaaaaaaa)
			color-mix(in hsl, var(--aaaaaaaaaaaa) var(--aaaaaaaaaaaaaa), transparent)
		);
	}
</style>
