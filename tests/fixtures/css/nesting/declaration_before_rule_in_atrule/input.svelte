<style>
	@media screen {
		.a {
			color: red;
			.b {
				color: blue;
			}
		}
		.c {
			margin: 0;
			@media print {
				color: green;
			}
		}
		.e {
			padding: 0;

			.f {
				color: red;
			}
		}
	}
</style>
