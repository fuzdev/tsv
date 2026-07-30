<style>
	/* A comment glued to a value takes a space on each side */
	@supports  ( display:  grid   /* c */ )  {
		div  {
			color:  red;
		}
	}

	/* The space after `:` is decided by the property name, past the comment */
	@supports  ( gap   /* c */  :  1px )  {
		div  {
			color:  red;
		}
	}

	/* A comment trailing a nested condition */
	@supports  ( not  ( display:  grid )   /* c */ )  {
		div  {
			color:  red;
		}
	}

	/* One space before a nested connector, not two */
	@supports  ( ( display:  grid )   /* c */   and  ( gap:  1px ) )  {
		div  {
			color:  red;
		}
	}

	/* Two comments in the run stay one space apart */
	@supports  ( ( display:  grid )   /* c */   /* d */   and  ( gap:  1px ) )  {
		div  {
			color:  red;
		}
	}

	/* The same rule inside a function argument */
	@supports  ( foo( a   /* c */   b ) )  {
		div  {
			color:  red;
		}
	}
</style>
