<style>
	/* an over-wide FIRST segment of a single query stays on the at-rule's line — nothing separates
	   it from the name — and the wrap lands at the `and` after it */
	@media (min-width: 999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999px) and
		(max-width: 2px) {
		div {
			color: red;
		}
	}
	@supports (a: 999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999) and
		(b: c) {
		div {
			color: red;
		}
	}
	/* a multi-line comment ending the first segment: the segment stays on the at-rule's line and
	   the segment after it is measured from where the comment ends */
	@media (min-width: 1px) /* aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
	b */ and (max-width: 2px) {
		div {
			color: red;
		}
	}
</style>
