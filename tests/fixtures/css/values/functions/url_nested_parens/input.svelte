<style>
	a {
		/* nested parens in unquoted url() are preserved verbatim */
		background: url(a(b));
		/* deeper nesting */
		background: url(a(b(c)));
		/* inner group with a comma and space */
		background: url(a(b, c));
		/* content after the inner group */
		background: url(a(b)c);
	}
</style>
