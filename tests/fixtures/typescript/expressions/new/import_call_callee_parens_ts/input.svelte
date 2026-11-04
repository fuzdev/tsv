<script lang="ts">
	// a `!` on the import, or type arguments after the pair, keep the `new` callee pair
	const r1 = new (import(a)!)();
	const r2 = new (import(a)!.b)();
	const r3 = new (import(a))<T>();
	// a pair the callee already prints needs no second one
	const r4 = new (import(a)<T>).k();
</script>

{new (import(a)!.b)()}
