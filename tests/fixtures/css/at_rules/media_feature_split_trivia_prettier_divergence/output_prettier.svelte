<style>
	/* a comment is one token, so the feature name runs past the colon inside it and the
	name's number is kept */
	@media (/*: */ 1.5: a) {
		b {
			color: red;
		}
	}
	/* a string is one token for the same reason, and keeps its quote normalization */
	@media ('x: y' 1.5: a) and ('x: y': 2.5) {
		b {
			color: red;
		}
	}
</style>
