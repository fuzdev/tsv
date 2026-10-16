<script lang="ts">
	// A second type argument list after the list on an argument-less `new` callee is no list
	// at all: tsc takes none ahead of a `<`, so `new f<T><U>(x)` is the comparison
	// `(new f < T) > <U>(x)`, whose right operand is the type assertion `<U>(x)`. The pair
	// around the `<` operand is the spelling every parser reads as that comparison.
	new f<T>
		<U>(x);
	new a.b<T><U>(x);
	new f<T><U>(x)(y);
	new f<T><U>(x, y);
	// The assertion takes what follows its type, a template included.
	new f<T><U>`x`;
	const a = new f<T><U>(x);
	g(new f<T><U>(x));

	// The call twin reads the same way.
	f<T><U>(x);
	f<T><U>(x).y;
	f<T /* a */><U /* b */>(x).y();
	g(f<T><U>(x)`t`);
	new f<T><U>(x)()();
	f<T><U>`x`()();
	f<T><U>(x)`t`()();

	// A call or an access after the chain takes a pair around it.
	(f<T><U>(x))();
	(f<T><U>(x))()();
	(f<T><U>(x))(z);
	(f<T><U>(x))?.();
	(f<T><U>(x))<W>();
	(f<T><U>(x)!)();
	(f<T><U>(x).z)();
	(new f<T><U>`x`)()();
	(new f<T><U>`x`)?.()();
	(f<T><U>(x)`t`).y;
	new (f<T><U>(x)<T>)`t`();
	new (f<T><U>(x).k<T>)`t`();
	new (f<T><U>(x).k<T>)`t`.k();
	new (f<T><U>(x).k<T>)`t`()();
</script>

{new f<T><U>(x)}
{f<T><U>(x)}
{g(f<T><U>(x)`t`)}
{new f<T><U>(x)()()}
{f<T><U>`x`()()}
{(f<T><U>(x))()}
{(f<T><U>(x))?.()}
{(new f<T><U>`x`)()()}
{#if (f<T><U>(x))()}{/if}
{(f<T><U>(x)`t`).y}
{new (f<T><U>(x)<T>)`t`()}
{new (f<T><U>(x).k<T>)`t`()}
