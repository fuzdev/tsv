<script>
	/* trailing comment on the last item ends at column 100 - stays on the line */
	const  a  =  [  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  2  /* c */  ] ;

	/* trailing comment on the last item would end at column 101 - the item wraps with its comment */
	const  b  =  [  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,
		2  /* cc */  ] ;

	/* comment glued before the comma ends at column 100 (with the comma) - stays on the line */
	const  c  =  [  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  2  /* ccc */  ,
		3 ,  4  ] ;

	/* comment glued before the comma would end at column 101 - the item wraps with its comment */
	const  d  =  [  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,
		2  /* cccc */  ,  3 ,  4  ] ;

	/* leading comment glued to an item ends at column 100 (with the comma) - stays on the line */
	const  e  =  [  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  /* ccc */  2 ,
		3 ,  4  ] ;

	/* leading comment glued to an item would end at column 101 - the item wraps with its comment */
	const  f  =  [  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,  1 ,
		/* cccc */  2 ,  3 ,  4  ] ;
</script>
