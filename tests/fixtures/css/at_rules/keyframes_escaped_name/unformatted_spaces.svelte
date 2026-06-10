<style>
	/* escaped @ keeps its backslash (a bare @ would start an at-keyword) */
	@keyframes   \@mymove   {
		from   {
			top  :  0  ;
		}
	}

	/* escape mid-identifier is preserved */
	@keyframes   a\@b   {
		from   {
			top  :  0  ;
		}
	}

	/* hex escape with space terminator lets a name start with a digit */
	@keyframes   \31 23   {
		from   {
			top  :  0  ;
		}
	}
</style>
