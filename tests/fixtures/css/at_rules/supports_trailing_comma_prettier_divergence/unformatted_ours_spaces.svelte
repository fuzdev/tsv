<style>
	/* a trailing comma in a declaration value is kept, glued to the closing paren */
	@supports (x: a , ) {
		div {
			color: red;
		}
	}

	@supports (x: 'b', ) {
		div {
			color: blue;
		}
	}
</style>
