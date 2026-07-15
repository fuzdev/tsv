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

	/* Dash-match operator with flag */
	[lang|='en' i] {
		font-style: italic;
	}
	[lang|='en' s] {
		font-style: normal;
	}
</style>
