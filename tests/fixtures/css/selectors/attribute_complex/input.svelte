<style>
	/* With other selectors */
	div[class~='active']:hover {
		color: red;
	}

	/* Multiple attributes */
	input[type='text'][required] {
		border: 1px solid blue;
	}

	/* In complex selectors */
	.parent > [data-id^='item-'] {
		margin: 10px;
	}

	/* Combined with pseudo-classes */
	a[href^='https']:visited {
		color: purple;
	}

	/* Multiple operators on same element */
	[lang|='en'][dir='ltr'] {
		text-align: left;
	}

	/* With descendant combinator */
	nav [role='menuitem'] {
		padding: 5px;
	}
</style>
