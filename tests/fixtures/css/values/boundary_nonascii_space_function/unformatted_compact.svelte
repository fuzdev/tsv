<style>
	a {
		/* A no-break space (U+00A0) is not CSS whitespace, so one glued ahead of a function
		   name is value content: the run stays one opaque token, kept verbatim */
		width: calc(1px);
		color: var(--x);
		/* ...and between the name and its `(`: the same opaque token, not a function call */
		height:calc (1px);
		/* A keyword and a color led by one keep it the same way */
		color: red;
		color: rgb(1, 2, 3);
	}
</style>
