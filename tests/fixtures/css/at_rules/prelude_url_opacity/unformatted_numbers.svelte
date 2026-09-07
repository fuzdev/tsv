<style>
	/* an unquoted `url()` is opaque to the value reader and transparent to the media
	reader's number pass, so one authoring has two canonical forms */
	@supports (a: url(1.50)) {
		b {
			color: red;
		}
	}
	@media (a: url(1.50)) {
		b {
			color: red;
		}
	}
	/* the media reader's number pass runs in value position only, so a `url()` in name
	position keeps its content whichever reader is asked */
	@media (url(1.50): a) {
		b {
			color: red;
		}
	}
</style>
