<script lang="ts">
	// Object arg multiline + callback - triggers full expansion
	fn(  
		
	{
		a:1,
	} 	, 
	 (x)=>{
		return x;
	}  , 
 );

	// Multiple object properties
	fn( 
		
	{
		a:1,
		b:2,
	}  ,  (x)=>{
		return x;
	}  , 


);

	// Nested object
	fn(  {
		a:{

			b:1,
		},
	}  ,  (
		x)=>
		{
		return x;
	}  , 
	
	);
</script>
