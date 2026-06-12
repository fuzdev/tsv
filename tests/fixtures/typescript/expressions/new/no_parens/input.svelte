<script>
	// new without parentheses (prettier adds parens)
	const a = new Foo();
	const b = new a.b.C();

	// Chained after new
	const c = new Foo().bar;
	const d = new Foo().bar();

	// Precedence
	const e = new new Foo()();
	const f = new new Foo()();

	// With member expression target
	const g = new obj.Constructor();
	const h = new deep.nested.Class();

	// Function call then new
	const i = fn().Constructor;
</script>
