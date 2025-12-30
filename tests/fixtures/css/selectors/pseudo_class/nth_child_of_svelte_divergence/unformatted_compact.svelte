<style>
	/* CSS Selectors Level 4: :nth-child(An+B of S) */
	li:nth-child(2n of .class1) {
		background: lightgray;
	}

	div:nth-child(odd of .class2) {
		color: blue;
	}

	tr:nth-last-child(3 of .class3) {
		border-top: 2px solid red;
	}

	/* Complex selector after 'of' */
	span:nth-child(2n+1 of div.class4>.class5) {
		font-weight: bold;
	}
</style>
