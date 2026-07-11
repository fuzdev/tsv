<style>
	@scope (.a, , .b) {
		color: red;
	}

	@scope (.) to (.x, .) {
		color: blue;
	}

	@scope () to (.footer) {
		color: green;
	}

	@scope (.card) to () {
		color: gold;
	}

	@scope to () {
		color: teal;
	}
</style>
