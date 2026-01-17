<script lang="ts">
	// Logical expressions need parens
	const a = [...b || []];
	const c = [...d && e];
	const f = [...g ?? h];

	// Ternary needs parens
	const i = [...j ? k : l];

	// Assignment needs parens
	const m = [...n = o];

	// Sequence needs parens
	const p = [...(q, r)];

	// Binary expressions need parens (spread has higher precedence)
	const s = [...t + u];

	// Await needs parens
	async function afn() {
		return [...await b];
	}

	// Yield needs parens
	function* gfn() {
		return [...yield b];
	}

	// TypeScript type assertions need parens
	const bb = [...cc as any[]];
	const dd = [...ee satisfies any[]];
	const ff = [...<any[]>gg];

	// Simple expressions don't need parens
	const v = [...w];
	const x = [...y.z];
	const aa = [...fn()];
	const hh = [...ii!];
</script>
