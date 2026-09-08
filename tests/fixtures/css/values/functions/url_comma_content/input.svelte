<style>
	/* a comma inside an unquoted url() is content: the token is kept verbatim, only the
	padding inside its parens trims, wherever the value is read from text */
	a {
		b: /* c */ url(x,y);
		c: /* c */ url(x ,y);
	}
	@import 'a.css' screen url(x,y);
	@import 'a.css' url(x ,y);
	@import 'a.css' screen url(x,y) and (a: b);
</style>
