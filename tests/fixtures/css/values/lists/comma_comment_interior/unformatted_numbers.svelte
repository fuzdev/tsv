<style>
	a {
		/* Interior whitespace, an interior comma, and normalizable tokens survive verbatim */
		transition: color .3s /* keep   this,  #FFF  .5px */, background .30s;
		/* An interior whitespace run alone */
		transition: color .3s /* a   b */, background .30s;
		/* A comment before the first element content is between the colon and the value, so
		   single-node elements stay inline */
		font-family: /* c */ a, b;
		/* A comment on a later element makes that element multi-node, so the list breaks */
		font-family: a, /* c */ b;
		/* When the list does break, that between comment stays on the colon line */
		font-family: /* c */ color .30s, b;
		/* A run of them, and one that belongs to an element, keep their own positions */
		font-family: /* c */ /* d */ color .3s, b;
		transition: /* c */ color .3s /* d */, background .30s;
	}
</style>
