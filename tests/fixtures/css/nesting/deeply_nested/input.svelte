<style>
	/* Deeply nested rules (4 levels) */
	.class1 {
		color: red;

		& .class2 {
			color: blue;

			& .class3 {
				color: green;

				& .class4 {
					color: yellow;
				}
			}
		}
	}
</style>
