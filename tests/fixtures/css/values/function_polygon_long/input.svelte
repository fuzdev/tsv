<style>
	div {
		/* short - stays inline */
		clip-path: polygon(0% 0%, 10% 0%, 10% 10%, 0% 10%);
		/* medium - stays inline (92 chars) */
		clip-path: polygon(0% 0%, 10% 0%, 10% 10%, 9% 10%, 9% 9%, 8% 9%, 8% 10%, 7% 10%, 7% 9%);
		/* long - wraps (exceeds 100 chars) */
		clip-path: polygon(
			50% 0%,
			61% 35%,
			98% 35%,
			68% 57%,
			79% 91%,
			50% 70%,
			21% 91%,
			32% 57%,
			2% 35%,
			39% 35%
		);
	}
</style>
