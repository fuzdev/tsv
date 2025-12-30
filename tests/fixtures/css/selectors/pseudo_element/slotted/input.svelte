<style>
	::slotted(*) {
		color: red;
	}

	::slotted(div) {
		color: blue;
	}

	::slotted(.class1) {
		color: green;
	}

	::slotted(div.class1#id1:hover) {
		color: yellow;
	}

	::slotted([slot]) {
		color: purple;
	}
</style>
