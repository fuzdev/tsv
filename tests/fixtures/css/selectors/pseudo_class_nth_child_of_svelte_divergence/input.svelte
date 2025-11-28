<style>
	/* CSS Selectors Level 4: :nth-child(An+B of S) */
	li:nth-child(2n of .item) {
		background: lightgray;
	}

	div:nth-child(odd of .highlight) {
		color: blue;
	}

	tr:nth-last-child(3 of .active) {
		border-top: 2px solid red;
	}

	/* Complex selector after 'of' */
	span:nth-child(2n + 1 of div.parent > .child) {
		font-weight: bold;
	}
</style>
