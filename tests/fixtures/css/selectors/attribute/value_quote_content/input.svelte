<style>
	/* value contains the other quote kind: delimiter preserved to avoid escaping */
	[data-attr1="it's"] {
		color: red;
	}

	[data-attr2='say "hi"'] {
		color: blue;
	}

	/* tie: both quote kinds inside — preferred single wins, its own occurrences escaped */
	[data-attr3='a\'b"c'] {
		color: green;
	}

	/* only the delimiter itself inside */
	[data-attr4='"'] {
		color: purple;
	}

	[data-attr5="'"] {
		color: orange;
	}

	/* hex escape in a string value stays verbatim */
	[data-attr6='\27 x'] {
		color: gray;
	}
</style>
