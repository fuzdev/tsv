<style>
	/* An unrecognized at-rule's prelude is an opaque token stream (CSS Syntax 3 §5.4.2), so tsv keeps it verbatim — whitespace runs, quote spelling and combinator spacing alike. Prettier re-parses the prelude of any at-rule on its hardcoded SCSS list; for @at-root that means parsing it as a SELECTOR (see README). */
	/* Whitespace run, quote spelling, combinator spacing — prettier normalizes all three */
	@at-root input[type='radio'] > .class1 {
		color: red;
	}
	/* An authored newline inside the prelude — prettier collapses it to one space */
	@at-root foo bar {
		color: red;
	}
	/* Control: identical prelude under a name NOT on prettier's list — both keep it verbatim */
	@unknown foo   bar {
		color: red;
	}
</style>
