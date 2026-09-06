<style>
	/* an unquoted url() is an opaque url-token: an escaped `)` is content, so the
	bytes after it are neither number- nor hex-normalized */
	@supports (a: url(x\)1.50)) and (b: url(x\)y#FFF)) {
		c {
			color: red;
		}
	}
	/* control: the escape-free url is preserved the same way */
	@supports (a: url(x1.50)) and (b: c) {
		c {
			color: red;
		}
	}
</style>
