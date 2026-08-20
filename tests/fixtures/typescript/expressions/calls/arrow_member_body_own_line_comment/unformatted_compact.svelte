<script>
	// an own-line comment between `=>` and a body prettier cannot expand — a member
	// expression — breaks every argument out instead of hugging the last one
	fn((e)=>
// c
e.prop);

	on(a,'b',(e)=>
// c
e.prop);

	// an identifier, binary or template body reads the same
	fn(a,(e)=>
// c
e);

	fn(a,(e)=>
// c
e+e);

	fn(a,(e)=>
// c
`text`);

	// the same rule in the `new` and member-chain spellings
	new A(a,(e)=>
// c
e.prop);

	obj.fn1(1).fn2(a,(e)=>
// c
e.prop);

	// an own-line block comment behaves the same
	fn(a,(e)=>
/* c */
e.prop);

	// a curried argument asks the gap of its innermost `=>`
	fn(a,()=>(e)=>
// c
e.prop);

	// control: a glued comment keeps the body hugged to `=>`
	fn(a,(e)=>/* c */e.prop);

	// control: a body prettier expands keeps the argument hugged to `(`
	fn(a,(e)=>
// c
({a:1}));
</script>
