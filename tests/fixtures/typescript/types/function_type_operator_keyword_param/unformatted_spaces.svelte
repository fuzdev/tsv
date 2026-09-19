<script lang="ts">
	// Type-operator words (readonly, keyof, infer, unique) used as parameter names
	// in a function type — a bare name before `)` then `=>`
	type F1 = ( readonly )  =>  U ;
	type F2 = ( keyof
	)  =>  U ;
	type F3 = (
		infer ) =>  U ;
	type F4 = ( unique )  =>  U ;

	// Annotated, optional, and listed
	type F5 = ( readonly  :  T )  =>  U ;
	type F6 = ( keyof  ?  :  T )  =>  U ;
	type F7 = ( infer  ,  b )  =>  U ;
	type F8 = ( unique  :  T ,  keyof  :  U ,  readonly  ?  :  V )  =>  W ;

	// In an annotation, nested, and as a type argument
	let v  :  ( readonly )  =>  U ;
	type F9 = ( a  :  ( keyof )  =>  U )  =>  V ;
	type F10 = ( ( infer )  =>  U ) [ ] ;
	fn < ( readonly )  =>  U > ( x ) ;
	fn < ( unique  :  T )  =>  U ,  V > ( x ) ;

	// A comment between the name and the `)`
	type F11 = ( readonly  /* c */  )  =>  U ;

	// Distinct from the parenthesized operator types
	type P1 = ( readonly  T [ ] ) [ ] ;
	type P2 = ( keyof  T ) [ ] ;
	type P3 = A  extends  ( infer  B ) [ ]  ?  B  :  never ;
</script>
