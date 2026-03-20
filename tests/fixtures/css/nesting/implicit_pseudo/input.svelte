<style>
	.class1 {
		color: red;

		span:hover {
			color: blue;
		}

		a:not(:last-child) {
			color: green;
		}

		span:not(:last-child)::after {
			content: ' > ';
		}

		div:first-child {
			margin: 0;
		}

		p:nth-child(2n + 1) {
			background: red;
		}

		/* Type selector with pseudo inside nested @media */
		@media (min-width: 800px) {
			h2 {
				font-size: 2rem;
			}

			span:hover {
				color: green;
			}
		}
	}
</style>
