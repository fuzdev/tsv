<style>
	/* every operator binds its operand to the RIGHT, so one with nothing after it is the
	condition's own last word: it is kept, and it takes no separator */
	@supports (a:b)and;
	@supports (a:b)and(c:d)or;
	@supports (a:b)not;
	@supports and;

	/* the whole run is kept, in source order and source case */
	@supports (a:b)AND OR NOT;
	@supports (a:b)/* c */and;
	@supports (a:b)and/* c */;

	/* inside a group the same word has no operand either, so it takes no space before `)` */
	@supports ((a:b)and);
	@import 'a.css' supports((a:b)and);

	/* control: an operator that joins two operands keeps its separator */
	@supports (a:b)and(c:d){.class{color:red;}}

	@container (a: b) and{.class{color:red;}}
</style>
