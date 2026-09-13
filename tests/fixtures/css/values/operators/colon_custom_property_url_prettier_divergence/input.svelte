<style>
	div {
		/* a URL-shaped custom-property value: its `://` is a top-level colon like any
		other, and the rest of the value is one word */
		--a: https: //a.fuz.dev/b;
		/* the same URL inside a url(), whose content is opaque to both formatters */
		--b: url(https://a.fuz.dev/b);
	}
</style>
