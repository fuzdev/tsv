<style>
	a {
		/* U+00A0 (no-break space) is not CSS whitespace: it stays inside the value
		   token, while a real ASCII space still splits the list */
		animation-name:   u v   ;
		transition:   u v   1s   ;
		/* …and at the END of a value or a function argument, where a Unicode trim
		   would silently eat it and rewrite the token */
		width:   u   ;
		transform:   translate(  u   )  ;
	}

	/* the same in a selector: a trailing NBSP is part of the class name, so eating it
	   would silently rename the class (and here, glue it to the list comma) */
	.u   ,
	.v   {
		color:   red;
	}
</style>
