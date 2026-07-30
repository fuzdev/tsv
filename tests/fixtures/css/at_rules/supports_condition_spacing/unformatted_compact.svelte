<style>
	/* A value spelled `and` is a value keyword, not a boolean operator */
	@supports (font-family:and){div{color:red;}}

	/* A boolean operator between two nested conditions does space */
	@supports ((display:grid)and(color:red)){div{color:red;}}

	@supports (not(display:grid)){div{color:red;}}

	/* The space after `:` is decided by the property name, past the comment */
	@supports (display /* c */:grid){div{color:red;}}

	/* A connector after a selector() part */
	@supports selector(div:hover)and(display:grid){div{color:red;}}
</style>
