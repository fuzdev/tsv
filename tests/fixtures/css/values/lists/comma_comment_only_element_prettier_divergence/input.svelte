<style>
	a {
		/* A first element that is nothing but comments: tsv keeps the list inline */
		font-family: /* c */, b;
		/* Same for a run of them, and with a later element that would break the list */
		font-family:
			/* c */ /* d */,
			b b;
	}
</style>
