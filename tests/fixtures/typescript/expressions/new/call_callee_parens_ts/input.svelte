<script lang="ts">
	// a `!` on either side of a tag over a call keeps the `new` callee pair
	const r1 = new (f()!`x`)();
	const r2 = new (f()`x`!)();
	// a call inside a pair the callee already prints needs no second pair
	const r3 = new (f()<T>).k();
	const r4 = new (f()<T>)[0]();
	const r5 = new (f()<T>).k.m();
	const r6 = new (f()?.z).k();
	const r7 = new (a?.b())!.c();
</script>

{new (f()<T>).k()}
