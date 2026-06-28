<style>
	/* comment spacing inside a forgiving list (:is/:where/:not/:has) normalizes to single spaces */
	:is(.a,/* mid */.b) {
		color: red;
	}

	/* a comment before the comma keeps its side; spacing still normalizes */
	.c/* before */,.d {
		color: blue;
	}
</style>
