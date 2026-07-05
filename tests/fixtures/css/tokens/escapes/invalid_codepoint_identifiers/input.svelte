<style>
	div {
		/* CSS Syntax 3 §4.3.7: an escape whose code point is zero, is a surrogate
		   (U+D800–U+DFFF), or is above the maximum allowed code point (U+10FFFF)
		   decodes to U+FFFD REPLACEMENT CHARACTER rather than being a parse error;
		   the neighbouring in-range code points decode normally. Names are emitted
		   from raw source, so only the accept/reject verdict is under test here. */
		--\0: a;
		--\d7ff: b;
		--\d800: c;
		--\dfff: d;
		--\e000: e;
		--\10ffff: f;
		--\110000: g;
		/* The same rule applies inside a var() reference */
		color: var(--\d800);
		background: var(--\ffffff);
	}

	/* ...and inside an @supports condition */
	@supports (color: var(--\d800)) {
		div {
			color: green;
		}
	}
</style>
