<script>
	// Comment before arrow paren - should NOT be duplicated inside
	let a =  /* c */  ( x )  =>  x;

	// Async variant
	let b =  /* c */  async  ( x ) => x;

	// Multiple params with comment before paren
	let c =  /* c */  ( x ,  y ) => x + y;

	// Empty params
	let d =  /* c */  ( ) => 1;
</script>
