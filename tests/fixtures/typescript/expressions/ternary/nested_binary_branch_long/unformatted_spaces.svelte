<script>
	// A ternary under `return` indents its own branch chains; a nested ternary's branch chain
	// stays flush with its first operand (the grandparent is the outer ternary, not the return)
	function  fn1( )  {
		return  a  ?  b  :  c  ?  d  ?  e  :  f  &&
			( gggggggggggggggggggggggggggggg  ||  hhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhh  ||  iiiiiiiiiiiiiiiiiiiii )  :  false ;
	}

	// The same chain as a call argument
	fn( a  ?  b  :  c  ?  d  &&
		( gggggggggggggggggggggggggggggg  ||  hhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhh  ||  iiiiiiiiiiiiiiiiiiiiiii )  :  e ) ;

	// A nested consequent's branch
	function  fn2( )  {
		return  a  ?  b  ?  cccccccccccccccccccccccccccccccc  &&
			dddddddddddddddddddddddddddddddddddddd  &&  eeeeeeeeeeeeeeeeee  :  d  :  e ;
	}

	// 100 chars - the nested branch chain stays inline
	function  fn3( )  {
		return  a  ?  b  :  c  ?  d  :  eeeeeeeeeeeeeeeeeeeeeeeeeeeeeee  &&  fffffffffffffffffffffffffffffffff  &&  ggggggggggggggggg ;
	}

	// 101 chars - breaks, flush
	function  fn4( )  {
		return  a  ?  b  :  c  ?  d  :  eeeeeeeeeeeeeeeeeeeeeeeeeeeeeee  &&
			fffffffffffffffffffffffffffffffff  &&  gggggggggggggggggg ;
	}
</script>
