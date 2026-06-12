<style>
	:host {
		display: block;
	}

	:host(.class1) {
		color: red;
	}

	:host-context(.class2) {
		background: black;
	}

	:host-context(.class2) .class3 {
		color: white;
	}
</style>
