<style>
	::slotted(*) {
		color: red;
	}

	::slotted(div) {
		color: blue;
	}

	::slotted(.foo) {
		color: green;
	}

	::slotted(div.foo#bar:hover) {
		color: yellow;
	}

	::slotted([slot]) {
		color: purple;
	}
</style>
