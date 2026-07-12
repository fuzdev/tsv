<style>
	/* Exact match */
	[attr='value'] {
		color: red;
	}

	/* Prefix match */
	[href^='https'] {
		color: blue;
	}

	/* Suffix match */
	[href$='.pdf'] {
		color: green;
	}

	/* Substring match */
	[title*='example'] {
		color: yellow;
	}

	/* Whitespace list contains */
	[class~='active'] {
		color: purple;
	}

	/* Dash prefix match */
	[lang|='en'] {
		color: orange;
	}

	/* Exact match, bare attribute selectors */
	[type='text'] {
		border: 1px solid black;
	}
	[data-id='item'] {
		margin: 10px;
	}
</style>
