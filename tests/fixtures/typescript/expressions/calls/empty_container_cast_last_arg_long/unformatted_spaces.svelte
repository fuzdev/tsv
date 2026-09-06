<script lang="ts">
	// Prettier's `couldExpandArg` hugs an object or array last argument only when it is
	// non-empty or carries a comment, and reads that through a cast. An EMPTY literal under
	// a cast has nothing to expand: the hug would break the cast's `<...>` instead, so the
	// arguments break out.

	// 100 - fits
	aaaa.bbbb( ( acc ,  curr )  =>  acc.concat( curr.ccccccccccccccccccc ) ,  < Ddddddddddddddddddddddddddd[] >[ ] ) ;

	// 101 - every argument on its own line, the cast whole
	aaaa.bbbb(
		( acc ,  curr )  =>  acc.concat( curr.ccccccccccccccccccc ) ,
		< Dddddddddddddddddddddddddddd[] >[ ]
	) ;
	aaaa.bbbb(
		( acc ,  curr )  =>  acc.concat( curr.cccccccccccccccccccc ) ,
		[ ]  as  Ddddddddddddddddddddddddd[]
	) ;
	aaaa.bbbb(
		( acc ,  curr )  =>  acc.concat( curr.cccccccccccccccccc ) ,
		< Ddddddddddddddddddddddddddddddd >{ }
	) ;

	// the chain spelling of the same call
	aaaa( bbbb ,  ccccccccccccccc )?.reduce(
		( acc ,  curr )  =>  acc.concat( curr.eeee ) ,
		< Ddddddddddddddddd[] >[ ]
	) ;

	// a comment between the cast and the literal attaches to the LITERAL, so the same
	// clause makes an empty one expandable and the cast's `<...>` is what breaks

	// 100 - fits
	aaaa.bbbb( ( acc ,  curr )  =>  acc.concat( curr.ccccccccccccccccc ) ,  < Ddddddddddddddddddddd[] >/* c */ [ ] ) ;

	// 101 - the cast breaks, the literal keeps the `>` line
	aaaa.bbbb( ( acc ,  curr )  =>  acc.concat( curr.cccccccccccccccccc ) ,  <
		Ddddddddddddddddddddd[]
	>/* c */ [ ] ) ;
	aaaa.bbbb( ( acc ,  curr )  =>  acc.concat( curr.ccccccccccccccccc ) ,  <
		Dddddddddddddddddddddddd
	>/* c */ { } ) ;

	// a NON-EMPTY literal under the cast still hugs, and so does an empty one holding a comment
	aaaa.bbbb( ( acc ,  curr )  =>  acc.concat( curr.ccccccccccccccccccc ) ,  < Ddddddddddddddddddddddddddd[] >[
		1
	] ) ;
	aaaa.bbbb( ( acc ,  curr )  =>  acc.concat( curr.cccccccccccccccccccc ) ,  < Dddddddddddddddddddd[] >[
		/* c */
	] ) ;
</script>
