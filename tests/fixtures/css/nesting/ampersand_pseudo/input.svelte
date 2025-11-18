<style>
	.button {
		background: white;

		&:hover {
			background: gray;
		}

		&:focus {
			outline: 2px solid blue;
		}

		&::before {
			content: '>';
		}

		&::after {
			content: '<';
		}
	}
</style>
