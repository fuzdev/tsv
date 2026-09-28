<script>
	// a `new` callee that contains a call needs parens, else the args bind to the inner call
	const r1 = new (f())();
	const r2 = new (a.b())();
	const r3 = new (f().C)();
	const r4 = new (a?.b())();
	// a tag over a call is such a callee too
	const r5 = new (f()`x`)();
	const r6 = new (a.b()`x`.c)();
	// a call inside a pair the callee already prints needs no second pair
	const r7 = new (a?.b()).c();
	const r8 = new (a?.b())`x`();
</script>

{new (f()`x`)()}
{new (a?.b()).c()}
