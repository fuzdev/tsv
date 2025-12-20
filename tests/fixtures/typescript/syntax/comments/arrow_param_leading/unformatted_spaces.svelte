<script>
	// Comment before arrow paren - should NOT be duplicated inside
	let a =  /* c */  ( x )  =>  x;

	// Comment inside paren - should be preserved
	let b = (  /* c */  x ) => x;

	// Both comments - each in correct position
	let c =  /* outer */  (  /* inner */  x ) => x;

	// Async variant
	let d =  /* c */  async  ( x ) => x;

	// Multiple params with comment before first
	let e =  /* c */  ( x ,  y ) => x + y;
</script>
