<style>
	/* the two prelude readers disagree about separator and operator spacing: the value
	reader groups the value and prints its own, so a comma takes `, ` and an operator
	takes ` + `; the media reader keeps the media-value's raw text verbatim */
	@supports (a: x , 9px) and (b: #fff + 5) {
		c {
			color: red;
		}
	}
	@media (a: x,9px) and (b: #FFF+5) {
		c {
			color: red;
		}
	}
	/* a grouped condition is name position whole, so its runs collapse to one space
	while the spaces beside its inner parens stay */
	@media (not ( screen and ( color ) )) {
		c {
			color: red;
		}
	}
</style>
