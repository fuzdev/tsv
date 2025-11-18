<style>
	@property --prop1 {
		syntax: '<length>';
		inherits: true;
		initial-value: 100px;
	}

	@property --prop2 {
		syntax: '<color>';
		inherits: false;
		initial-value: blue;
	}

	@property --prop3 {
		syntax: '<percentage> | <number>';
		inherits: true;
		initial-value: 50%;
	}

	div {
		width: var(--prop1);
		color: var(--prop2);
		opacity: var(--prop3);
	}
</style>
