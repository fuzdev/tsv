<style>
	.a {
		@media print {
			color: red;
		} /* trailing1 */
		color: blue;
	}
	@supports (display: grid) {
		.b {
			@media print {
				color: green;
			} /* trailing2 */
			margin: 0;
		}
	}
</style>
