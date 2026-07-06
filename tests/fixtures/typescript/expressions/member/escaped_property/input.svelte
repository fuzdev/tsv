<script lang="ts">
	// Property names decode from escapes: \u0061 -> a, \u0069n -> in, \u{63} -> c
	const a = x.a;
	const b = x?.a;
	const c = x.in;
	const d = x.a.b;
	const e = new X.a();
	const f = y.c;
</script>
