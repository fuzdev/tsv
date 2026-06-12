<script lang="ts">
	// Definite assignment without type annotation (parses; TS reports a semantic error only)
	let  a !  ;

	// With initializer
	let  b !  =  x ;
	var  c !  =  x ;

	// Multiple declarators
	let  d ! ,
		e !  =  x ;

	// Mixed annotated and unannotated
	let  f ! :  A ,
		g !  =  x ;

	// Exported
	export  const  h !  =  x ;
</script>
