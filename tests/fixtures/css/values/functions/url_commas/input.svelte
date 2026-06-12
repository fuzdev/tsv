<style>
	a {
		/* trailing comma preserved verbatim in unquoted url() */
		src: url(a,b,);
		/* consecutive empty segment preserved */
		src: url(c,,d);
		/* query string with a trailing comma */
		src: url(e.ttf?list=f,g,);
	}
</style>
