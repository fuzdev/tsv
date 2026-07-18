<style>
	/* An escaped control char is part of the at-rule NAME token (parseCss decodes
	   the whole run to the name; postcss/prettier instead splits it into name +
	   prelude and prints `@m \A x {`). tsv preserves the source escape verbatim so
	   the decoded control char never lands raw in the output. */
	@m\A x {
		a {
			x: y;
		}
	}
</style>
