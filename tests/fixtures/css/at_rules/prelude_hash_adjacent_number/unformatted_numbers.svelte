<style>
	/* a number abutting the `#`-token before it: the value path keeps the pair as one
	word, the media path re-splits it and normalizes the number */
	@supports (a: #1.50) and (b: #1.50px) {
		c {
			color: red;
		}
	}
	@media (a: #1.50) and (b: #1.50px) {
		c {
			color: red;
		}
	}
	/* a `#` with no ident content after it heads no token, so the number normalizes on
	both paths */
	@supports (a: #.50) and (b: #ABCDE) {
		c {
			color: red;
		}
	}
	@media (a: #.50) and (b: #ABCDE) {
		c {
			color: red;
		}
	}
	/* the same extent decides the hex fold: a colour is a colour only when the token
	ends at its digits, so a word running on past them is not one */
	@supports (a: #FFF.5) and (b: #FFF/5) and (c: #FFFé) and (d: #FFF 5) {
		e {
			color: red;
		}
	}
	@media (a: #FFF.5) and (b: #FFF/5) and (c: #FFFé) and (d: #FFF 5) {
		e {
			color: red;
		}
	}
</style>
