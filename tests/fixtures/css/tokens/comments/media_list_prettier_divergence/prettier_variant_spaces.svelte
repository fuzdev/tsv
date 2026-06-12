<style>
	/* Multiple comment positions - we normalize spacing, Prettier preserves input */
	@media /* a */ screen /* b */ and /* c */ (min-width: 500px) {
		div {
			color: red;
		}
	}

	/* Comment with or keyword */
	@media (width > 100px) /* d */ or /* e */ (height > 200px) {
		div {
			color: blue;
		}
	}

	/* Comment inside condition value */
	@media (min-width: /* size */  500px) {
		div {
			color: green;
		}
	}

	/* Comment with not keyword */
	@media not /* f */ screen {
		div {
			color: yellow;
		}
	}

	/* Comment with only keyword */
	@media only /* g */ screen {
		div {
			color: purple;
		}
	}

	/* Comment before comma in list */
	@media screen /* h */, print {
		div {
			color: orange;
		}
	}
</style>
