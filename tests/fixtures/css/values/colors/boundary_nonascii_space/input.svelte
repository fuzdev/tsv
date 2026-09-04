<style>
	a {
		/* A no-break space (U+00A0) is not CSS whitespace, so one inside a color function's
		   arguments is argument content: the value is not a color, and the function's
		   arguments are kept verbatim rather than rebuilt from parsed channels */
		color: rgb( 1, 2, 3);
		color: rgb(1,  2, 3);
		color: rgb(1, 2, 3 );
		color: rgb(1 2 3 /  0.5);
		color: hsl( 120deg, 50%, 50%);
	}
</style>
