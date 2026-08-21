<script lang="ts">
	// Every ECMAScript `WhiteSpace` member separates an arrow's parts, not just
	// `[ \t\n\r]` — the `unformatted_*` variants carry VT, FF, NBSP, ZWNBSP and the
	// `Zs` category, and both formatters normalize them to the plain spaces below.
	// `LineTerminator` is the separate production the `input_invalid_*` files pin:
	// `[no LineTerminator here]` before `=>` rejects LS and PS as it rejects LF.
	const f = (a)=> a + 1;
	const h = (a): number => a + 1;
	const j = async (a)=> a + 1;
	const k = <T, U>(a: T, b: U)=> a;
	const m = ()=> 1;
	const n = arr.map((a)=> a);
</script>
