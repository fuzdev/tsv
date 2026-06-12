<style>
	.class1 {
		padding: 20px;
		background: white;

		.class2 {
			font-size: 24px;

			&:hover {
				color: blue;
			}
		}

		& .class3 {
			font-size: 12px;
		}

		& > .class4 {
			margin: 10px 0;

			@media (min-width: 768px) {
				margin: 20px 0;
			}
		}

		&.class5 {
			border: 2px solid blue;

			@supports (box-shadow: 0 0 5px blue) {
				box-shadow: 0 0 5px blue;
			}
		}
	}
</style>
