<style>
	/* Case-insensitive flag */
	[attr='value' i] {
		color: red;
	}

	/* Case-sensitive flag */
	[attr='value' s] {
		color: blue;
	}

	/* No flag (default) */
	[attr='value'] {
		color: green;
	}

	/* Flag with different operators */
	[href^='https' i] {
		text-decoration: underline;
	}
	[title*='example' s] {
		font-weight: bold;
	}
</style>
