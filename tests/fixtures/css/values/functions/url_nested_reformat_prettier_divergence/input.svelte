<style>
	a {
		/* unquoted url() content is an opaque <url-token>; tsv keeps it verbatim, prettier adds a space after the comma inside the nested parens */
		background: url(a(b,c));
		/* prettier drops the whitespace inside the nested parens; tsv keeps it verbatim */
		background: url(a( b ));
	}
</style>
