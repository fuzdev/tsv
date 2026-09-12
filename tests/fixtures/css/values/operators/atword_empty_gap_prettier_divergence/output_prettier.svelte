<style>
	div {
		/* a lone `@` is an empty `@`-word; prettier welds it onto the member after it,
		turning a delim and an ident into one at-keyword — tsv keeps the authored gap */
		a: @a;
		b: @(2.5);
		/* controls, where the two agree: an authored glue is kept on both sides */
		c: @(2.5);
		d: a @(2.5);
	}
</style>
