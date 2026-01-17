<script lang="ts">
	// Object literal callback - call stays with =
	a  =  b.map((x) => ({ 
		   ...x,
		c:  true,

	}));

	// Block body callback - call stays with =
	a  =  b.map(( x ) =>  {


		 fn(x);
		return x  *  2;  


	});
</script>
