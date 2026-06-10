<style>
	a {
		/* unquoted url() with a query string */
		src: url(a.ttf?query=1);
		/* query with multiple params */
		src: url(b.woff2?a=1&b=2);
		/* query with commas */
		src: url(c.ttf?list=a,b,c);
		/* query with escaped parens */
		src: url(d.woff2?foo=rgb\(255,255,0\));
	}
</style>
