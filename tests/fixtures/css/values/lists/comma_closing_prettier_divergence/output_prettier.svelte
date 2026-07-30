<style>
	div {
		/* a comma closing the value list is authored content, so it is kept */
		transition: a;
		/* the same, in a list that breaks one element per line */
		transition:
			a 1s ease,
			b 2s ease;
		/* a function's argument list takes the same rule */
		background: linear-gradient(red, blue);
		/* a custom property substitutes its tokens verbatim */
		--x: a;
		/* and the rule reaches a function nested in one */
		--y: rgb(1, 2, 3);
	}
</style>
