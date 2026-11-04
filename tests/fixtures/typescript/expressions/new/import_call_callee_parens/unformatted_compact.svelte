<script>
	// a dynamic import is a call: as a `new` callee it keeps the pair, as bare `new import(a)()`
	// is a syntax error
	const r1=new(import(a))();
	const r2=new(import(a).B)(b);
	const r3=new(import(a)[0])();
	// a call further along the callee takes the same one pair
	const r4=new(import(a).then(f))();
</script>

{new(import(a).B)()}
