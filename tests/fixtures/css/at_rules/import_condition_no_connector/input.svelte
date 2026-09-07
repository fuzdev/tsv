<style>
	/* each of these is the prelude's last value and a bare identifier run, so each routes
	to the media-query printer — the single place the prelude's value normalization runs */
	@import 'a.css' screen;
	@import 'b.css' layer;
	@import 'c.css' not (a: 0.5px);
	@import 'd.css' supports(display: grid) (a: 0.5px);

	/* none holds a top-level and/or, so none has a break point: the condition is emitted
	verbatim and overruns rather than wrapping, at 100 chars and at 101 */
	@import 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.css' (min-width: 768px);
	@import 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.css' (min-width: 768px);
</style>
