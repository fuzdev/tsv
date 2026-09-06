<style>
	/* an escape does not end an ident, so the feature name lowercases across it */
	@media (A\42 C: 1px) {
		d {
			color: red;
		}
	}
	/* a custom-media name is case-sensitive, so the whole run survives */
	@media (--A\42 C: 1px) {
		d {
			color: red;
		}
	}
	@media (--MIN\-WIDTH: 1px) {
		d {
			color: red;
		}
	}
</style>
