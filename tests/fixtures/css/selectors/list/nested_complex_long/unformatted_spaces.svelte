<style>
	/* 100 chars exactly - complex selector inside :is() stays inline (at boundary) */
	span:is(  .a  >  .b  >  .c  >  .d  >  .e  >  .f  >  .g  >  .h  >  .i  >  .j  >  .k  >  .l  >  .m  >  .n  >  .o  >  .p  >  .q  >  .r  ) {
		color:   red;
	}

	/* 101 chars - nested list wraps, complex selector stays on one line */
	span:is(  .a  >  .b  >  .c  >  .d  >  .e  >  .f  >  .g  >  .h  >  .i  >  .j  >  .k  >  .l  >  .m  >  .n  >  .o  >  .p  >  .q  >  .rr  ) {
		color:   red;
	}
</style>
