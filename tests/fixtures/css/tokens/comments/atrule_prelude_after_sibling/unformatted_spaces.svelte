<style>
	/* A prelude comment belongs to its own at-rule, never to the sibling before it */
	@import   'a.css'  ;
	@import   /* c1 */   'b.css'  ;
	@import   'c.css'   /* c2 */  ;
	div   {
		color:   red  ;
	}
	@supports   /* c3 */   (display:   grid)   {
		div   {
			color:   red  ;
		}
	}
	@container   /* c4 */ (width > 1px)   {
		div   {
			color:   red  ;
		}
	}
	@scope /* c10 */ (.class)   {
		color:   red  ;
	}

	/* A comment on the sibling's line but outside the at-rule still trails the sibling */
	div   {
		color:   blue  ;
	}   /* c5 */
	@supports   /* c6 */   /* c7 */   (display:   flex)   {
		div   {
			color:   red  ;
		}
	}
	span   {
		color:   blue  ;
	}   /* c8 */
	@supports   (display:   grid)   {
		div   {
			color:   red  ;
		}
	}

	/* Nested, the same seam inside a block */
	span   {
		color:   red  ;
		@supports   /* c9 */   (display:   grid)   {
			color:   blue  ;
		}
	}
</style>
