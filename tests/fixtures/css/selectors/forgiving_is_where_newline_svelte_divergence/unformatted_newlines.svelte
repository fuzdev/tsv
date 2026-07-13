<style>
	/* :is() forgiving list: the empty `.` class makes the complex selector invalid, so it is dropped from the AST; tsv keeps the dropped text but collapses its internal whitespace runs (incl. newlines) to single spaces */
	div:is(.a > .
		> .b) {
		color: red;
	}
</style>
