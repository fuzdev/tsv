<style>
	div {
		/* 100 chars - no wrap */
		prop: var(--a,/* comment */var(--b) cccccccccccccccccccccccccccccccccccccccccccccccccccccccc);
		/* 101 chars - wraps */
		prop: var(--a,/* comment */var(--b) ccccccccccccccccccccccccccccccccccccccccccccccccccccccccc);
	}
</style>
