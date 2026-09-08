<style>
	/* `url` is ASCII case-insensitive: `URL(x ,y)` is one opaque url-token whose content is
	kept verbatim, in a declaration and in an `@import` prelude alike */
	a {
		b: URL(x, y);
		c: /* c */ URL(x, y);
		d: Url(a, 0.5);
	}
	@import URL(x, y);
	@import 'a.css' URL(x, y);
	@import 'a.css' screen Url(a, 0.5);
</style>
