<script lang="ts">
	// Curried call with string + callback (test.each pattern)
	fn( 
		 'a'  )
		 (  'b'  ,   ()   =>   {

		fn();
	}  );

	// Curried call from method access (Jest/Vitest pattern)
	test.each( 
		 [  
			1  ,   
			2  ,   3  ]  
		)(
			  'name'  ,  
				 ()   =>
				    {

		fn();
	}  );

	// Curried call with multiple args before callback
	fn(  )(
		  'a'  ,
			   'b'  ,
				    ()   =>   {
		fn();
	}
);

	// Simple curried with single callback - stays inline
	fn(  )(  ()   =>   {
		fn();
	}  );

	// Regular call (non-curried) with callback - stays inline
	fn(  'a'  ,   ()   =>   {
		fn();
	}  );

	// Blank line between args - preserves expansion
	fn(  )(

		  'a'  ,
			   'b'  ,


				    ()   =>   {
			fn();
		}  ,
	);

	// Leading comment before first arg - forces expansion
	fn(  )(
		  // leading comment
		    'a'  ,
			     ()   =>   {
			fn();
		}  ,
	);
</script>
