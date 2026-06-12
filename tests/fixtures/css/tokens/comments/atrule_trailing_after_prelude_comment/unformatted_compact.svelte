<style>
	@import "a.css" /* prelude */;/* trailing */
	@namespace svg url(http://x) /* prelude */;/* ns-trailing */
	@supports (display: flex) /* prelude */ {
		div {
			color: red;
		}
	}/* supports-trailing */
	@media screen /* prelude */ {
		div {
			color: blue;
		}
	}/* media-trailing */
</style>
