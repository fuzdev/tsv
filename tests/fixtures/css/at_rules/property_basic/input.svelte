<style>
	@property --prop1 {
		syntax: '<color>';
		inherits: false;
		initial-value: red;
	}

	div {
		color: var(--prop1);
	}
</style>
