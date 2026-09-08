<style>
	/* the whole prelude is one group: at 100 chars it stays inline (the `;` counts) */
	@custom-selector   :--aaaaaaaaaaaaaaaaaaaaaaaa
		.class1,
		.class2,
		.class3,
		.class4,
		.class5,
		.class6  ;
	/* at 101 every selector goes to its own indented line and the name stays on the head line */
	@custom-selector   :--aaaaaaaaaaaaaaaaaaaaaaaaa   .class1,   .class2,   .class3,   .class4,   .class5,   .class6  ;
	/* a selector that overflows the broken list breaks at its combinator, indented once more */
	@custom-selector   :--a   .aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa   >   .bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb,   .class2  ;
	/* a long name alone breaks the list even when each selector is short */
	@custom-selector   :--aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa   h1,   h2  ;
</style>
