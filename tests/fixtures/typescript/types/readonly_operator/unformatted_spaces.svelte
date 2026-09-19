<script lang="ts">
	// `readonly` is always the type operator — it takes an operand, and is never
	// a type name of its own
	type A1 = readonly   T [ ] ;
	type A2 = readonly   [ A ,  B ] ;
	type A3 = readonly   [ ] ;
	type A4 = readonly   T [ ] [ ] ;
	type A5 = readonly   ( A  |  B ) [ ] ;
	type A6 = readonly   'a' [ ] ;

	// As a member, an argument, and an element
	type B1 = A  |  readonly   B [ ] ;
	type B2 = A  &  readonly   [ B ] ;
	type B3 = C < readonly   T [ ] > ;
	type B4 = ( readonly   T [ ] ) [ ] ;
	type B5 = ( a :  readonly   T [ ] )  =>  readonly   U [ ] ;
	let v = x  as  readonly   T [ ] ;
</script>
