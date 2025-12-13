<style>
	:host {
		display: block;
	}

	:host(.active) {
		color: red;
	}

	:host-context(.dark) {
		background: black;
	}

	:host-context(.dark) .inner {
		color: white;
	}
</style>
