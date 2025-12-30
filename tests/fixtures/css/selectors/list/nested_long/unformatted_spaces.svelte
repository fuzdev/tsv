<style>
	/* 100 chars exactly - stays inline (at boundary) */
	span:is(  
		.a,  .b,  .c,  .d,  .e,  .f,  .g,  .h,  .i,  .j,  .k,  .l,  .m,  .n,  .o,  .p,  .q,  .r,  .s,  .t,  .u,  .vv  ) {
		color:   red;
	}

	/* 101 chars - wraps (exceeds print_width) */
	span:is(  
		.a,  .b,  .c,  .d,  .e,  .f,  .g,  .h,  .i,  .j,  .k,  .l,  .m,  .n,  .o,  .p,  .q,  .r,  .s,  .t,  .u,  .vvv  ) {
		color:   red;
	}

	/* Long nested selector list in :where() - wraps when exceeding print width */
	div:where( 
		 .class1,  .class2,  .class3,  .class4,  .class5,  .class6,  .class7,  .class8,  .class9,  .class10  ) {
		color:   red;
	}
</style>
