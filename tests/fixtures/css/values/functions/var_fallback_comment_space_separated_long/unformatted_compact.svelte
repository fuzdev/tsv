<style>
	div {
		/* 100 chars - no wrap */
		prop: var(--a,/* comment */var(--b) ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc);
		/* 101 chars - wraps */
		prop: var(--a,/* comment */var(--b) cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc);
	}
</style>
