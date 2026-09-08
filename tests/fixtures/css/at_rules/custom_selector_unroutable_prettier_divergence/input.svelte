<style>
	/* a prelude that is not `:--name` + whitespace + a selector list is kept verbatim:
	a name with nothing after it, and a list with no name */
	@custom-selector :--a;
	@custom-selector h1,h2;
	/* the shortest extension name, `--` itself, is a name like any other, and so is one
	whose dashes are escaped (the name is the decoded identifier; the spelling is kept) */
	@custom-selector :-- h1, h2;
	@custom-selector :\-\-b h1, h2;
	/* the draft's block form: a bare name, then a declaration block */
	@custom-selector :--c {
		expansion: h1, h2;
	}
</style>
