<style>
	/* an identity-escaped `|` belongs to the prefix's NAME, not the <wq-name> separator */
	[a\|b|attr] {
		color: red;
	}

	/* the hex spelling of the same prefix */
	[a\7c b|attr] {
		color: blue;
	}

	/* an ordinary escape in the prefix survives verbatim, like the attribute name's */
	[a\41 b|attr] {
		color: green;
	}
</style>
