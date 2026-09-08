<script lang="ts">
	// A block comment glued ahead of a should-hug union's first member binds to that
	// member at a container's opening delimiter too — a type argument's `<`, a tuple's
	// `[`, a paren shell's `(`, `keyof (`, an indexed access's `[` — so the union no
	// longer hugs: once it overflows the container breaks and the union takes the
	// one-per-line form with the comment after the pipe.

	// Type arguments: 100 chars keeps the hug glued, 101 breaks the argument list
	let a  :  Foo  <  /* c */  {  aaaa  :  Aaaaaaaaa  ;  bbb  :  Bbbbbb  ;  cccccccccccccccccccc  :  Cccccccccccccc  }  |  null  >  ;
	let b  :  Foo  <  /* c */  {  aaaa  :  Aaaaaaaaa  ;  bbb  :  Bbbbbb  ;  cccccccccccccccccccc  :  Ccccccccccccccc  }  |  null  >  ;
	let c  :  Foo  <  |  /* c */  {  aaaa  :  Aaaaaaaaa  ;  bbb  :  Bbbbbb  ;  cccccccccccccccccccc  :  Cccccccccccccccccccccccccccccc  }  |  null  >  ;
	fn  <  |  /* c */  {  aaaa  :  Aaaaaaaaa  ;  bbb  :  Bbbbbb  ;  cccccccccccccccccccc  :  Cccccccccccccccccccccccccccccccccccc  ;  }  |  null  >  (  )  ;

	// Tuple
	let d  :  [  /* c */  {  aaaa  :  Aaaaaaaaa  ;  bbb  :  Bbbbbb  ;  cccccccccccccccccccc  :  Ccccccccccccccccccccccc  }  |  null  ]  ;
	let e  :  [  |  /* c */  {  aaaa  :  Aaaaaaaaa  ;  bbb  :  Bbbbbb  ;  cccccccccccccccccccc  :  Cccccccccccccccccccccccccccccccc  ;  }  |  null  ]  ;

	// Paren shell and keyof
	let f  :  (  /* c */  {  aaaa  :  Aaaaaaaaa  ;  bbb  :  Bbbbbb  ;  cccccccccccccccccccc  :  Cccccccccccccccccccccc  }  |  null  )  [  ]  ;
	let g  :  (  |  /* c */  {  aaaa  :  Aaaaaaaaa  ;  bbb  :  Bbbbbb  ;  cccccccccccccccccccc  :  Cccccccccccccccccccccccccccccccc  ;  }  |  null  )  [  ]  ;
	let h  :  keyof  (  /* c */  {  aaaa  :  Aaaaaaaaa  ;  bbb  :  Bbbbbb  ;  cccccccccccccccccccc  :  Cccccccccccccccccc  }  |  null  )  ;
	let i  :  keyof  (  |  /* c */  {  aaaa  :  Aaaaaaaaa  ;  bbb  :  Bbbbbb  ;  cccccccccccccccccccc  :  Ccccccccccccccccccccccccccccc  }  |  null  )  ;

	// Indexed access
	let j  :  T  [  |  /* c */  {  aaaa  :  Aaaaaaaaa  ;  bbb  :  Bbbbbb  ;  cccccccccccccccccccc  :  Cccccccccccccccccccccccc  }  |  null  ]  ;
	let k  :  T  [  |  /* c */  {  aaaa  :  Aaaaaaaaa  ;  bbb  :  Bbbbbb  ;  cccccccccccccccccccc  :  Cccccccccccccccccccccccccccccccccc  ;  }  |  null  ]  ;

	// A comment ahead of the shell binds to the shell's owner, and the union keeps hugging
	let l  :  /* c */  (  {  aaaa  :  Aaaaaaaaa  ;  bbb  :  Bbbbbb  ;  cccccccccccccccccccc  :  Cccccccccccccccccccccccc  ;  }  |  null  )  [  ]  ;
</script>
