<style>
	/* and/or media-query connectors are case-insensitive; tsv preserves the
	   author's case (matching prettier), incl. @import preludes and comma lists. */
	@import url(reset.css) screen AND (min-width: 100px);
	@media screen AND (min-width: 100px) {
		a {
			color: red;
		}
	}
	@media (min-width: 100px) or (min-height: 100px), print {
		a {
			color: red;
		}
	}
</style>
