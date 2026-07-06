<script lang="ts">
	// Property names decode from escapes: \u0061 -> a, \u0069n -> in, \u{63} -> c
	const a = x.\u0061;
	const b = x?.\u0061;
	const c = x.\u0069n;
	const d = x.\u0061.b;
	const e = new X.\u0061();
	const f = y.\u{63};
</script>
