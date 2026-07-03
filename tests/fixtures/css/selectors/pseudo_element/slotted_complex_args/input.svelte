<style>
	/* bare number and An+B — the pseudo-arg Nth production */
	::slotted(0) {
		color: red;
	}

	::slotted(2n + 1) {
		color: blue;
	}

	/* descendant and child combinators — a full complex selector */
	::slotted(.class1 .class2) {
		color: green;
	}

	::slotted(.class1 > .class2) {
		color: yellow;
	}

	/* universal with a combinator */
	::slotted(* > *) {
		color: purple;
	}

	/* comma-separated selector list */
	::slotted(.class1, .class2) {
		color: orange;
	}
</style>
