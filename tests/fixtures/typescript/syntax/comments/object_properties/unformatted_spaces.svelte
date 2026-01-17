<script>
	// Simple trailing on first property
	const a = { x :  1  /* c1 */  ,  y :  2 };

	// Comment after comma (leading on second property)
	const b = { x :  1 ,  /* c2 */  y :  2 };

	// Both patterns combined
	const c = { x :  1  /* c1 */  ,  /* c2 */  y :  2 };

	// Leading on first property
	const d = { /* c1 */  x :  1 ,  y :  2 };

	// Comment between key and value
	const e = { x :  /* c1 */  1 ,  y :  2 };

	// Line comment after property
	const f = {

		x :  1 ,  // comment
		y :  2 ,

	};

	// Line comment before property
	const g = {

		x :  1 ,
		// comment
		y :  2 ,

	};

	// Block and line mixed
	const h = {

		x :  1  /* block */  ,
		y :  2 ,  // line
		z :  3 ,

	};

	// Shorthand property with comments
	const i = { x  /* after shorthand */  ,  y :  2 };

	// Spread with comments
	const j = { ...a  /* after spread */  ,  b :  2 };

	// Empty object with comment
	const k = {

		/* inside empty */

	};

	// Comment before closing brace
	const l = { x :  1 ,  y :  2  /* before close */ };

	// Computed property with comments
	const m = { [ /* inside brackets */  key ] :  1 };

	// Method with comments
	const n = { foo  /* after name */ ( ) { 
		
	} };
</script>
