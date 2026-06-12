<script lang="ts">
	// Simple typeof
	const  a :  typeof  x  =  x ;

	// Qualified name
	type  A  =  typeof  Foo . bar ;
	type  B  =  typeof  A . B . C ;

	// With type arguments
	const  b :  typeof  Array < string >  =  arr ;
	type  C  =  typeof  Map < string ,  number > ;

	// typeof import
	type  D  =  typeof  import ( './a' ) ;
	type  E  =  typeof  import ( './b' ) . T ;
	type  F  =  typeof  import ( './c' ) . A . B ;

	// In function return type
	function  fn ( ) :  typeof  x  {  return  x ;  }

	// In generic constraint
	type  G < T  extends  typeof  x >  =  { value :  T } ;

	// In union
	type  H  =  typeof  a  |  typeof  b ;

	// In intersection
	type  I  =  typeof  a  &  typeof  b ;

	// Conditional type with typeof
	type  J  =  typeof  x  extends  string  ?  typeof  y  :  typeof  z ;
</script>
