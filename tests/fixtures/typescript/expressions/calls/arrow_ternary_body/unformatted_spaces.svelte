<script lang="ts">
	// Short - arrow inline
	a  =  b.map(  
		( x )  =>  
		(
			 x
			   ?
				   c
					   :
						   d
							 )  );

	// Nested ternary
	a  =  b.map( 
		 ( x )   =>  ( 
			x  
			?
			
			y
			  :  
				
				z
				  ?
					  w
						
						: 
						 v
						
						)  );

	// With function calls
	a  =  b.map(  ( x )  =>  ( x  ?  fn( x )  :  fn( y ) )  );
</script>
