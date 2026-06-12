<style>
	/* No blank line before comment preceding nested rule */
	div {
		color: red;
		/* comment */
		span {
			margin: 0;
		}
	}

	/* No blank line before comment preceding nested at-rule */
	p {
		margin: 0;
		/* comment */
		@media (min-width: 767px) {
			margin: 1rem;
		}
	}

	/* Blank line before comment preceding nested rule preserved */
	a {
		color: red;

		/* comment */
		span {
			margin: 0;
		}
	}

	/* Blank line after comment before nested rule preserved */
	em {
		color: red;
		/* comment */

		&::after {
			content: ' ';
		}
	}
</style>
