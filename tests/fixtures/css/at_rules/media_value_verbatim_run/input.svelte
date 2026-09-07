<style>
	/* a `media-value` is a node prettier emits verbatim, so a whitespace run inside one
	is the author's: the run before a comment, and the run after an opaque `url()`, are
	kept rather than collapsed onto the separator's single space */
	@media (aspect-ratio: 16 / 9 /* c */) {
		div {
			color: red;
		}
	}

	@media (a: url(a.png)  b) {
		div {
			color: red;
		}
	}

	/* only the SPLITTING colon holds a run open against the expression's own `)` — the
	prettier-mandated space of an empty value; a later colon is value text, so the run
	after it trims like every other run beside that paren */
	@media (a: ) {
		div {
			color: red;
		}
	}

	@media (a: b:) {
		div {
			color: red;
		}
	}
</style>
