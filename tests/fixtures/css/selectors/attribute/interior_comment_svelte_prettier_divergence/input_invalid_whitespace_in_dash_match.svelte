<style>
	/* the `|=` spelling of the same boundary: the matcher's two components are the very
	   `|` that prefixes a namespace elsewhere, so a <whitespace-token> between them is
	   rejected here too - both parsers reject */
	[attr| ='value'] {
		color: red;
	}
</style>
