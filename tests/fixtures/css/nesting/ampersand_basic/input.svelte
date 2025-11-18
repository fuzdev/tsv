<style>
	.parent {
		color: red;

		& .child {
			color: blue;
		}

		&.active {
			color: green;
		}

		&[data-attr] {
			color: yellow;
		}
	}
</style>
