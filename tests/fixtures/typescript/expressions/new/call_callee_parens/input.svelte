<script>
	// a `new` callee that contains a call needs parens, else the args bind to the inner call
	const r1 = new (f())();
	const r2 = new (a.b())();
	const r3 = new (f().C)();
	const r4 = new (a?.b())();
</script>
