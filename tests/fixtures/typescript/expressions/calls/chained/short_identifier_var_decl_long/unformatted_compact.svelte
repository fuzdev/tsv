<script lang="ts">
	// Exactly 100 chars - stays inline
	export const aaaaaaaaaaa = z.fn1('type', [Aaaaaaaaaaaaa, Bbbbbbbbbbbbbb]).fn2({prop: 'valueeee'});

	// Exactly 101 chars - chain breaks after z
	export const aaaaaaaaaaaa = z.fn1('type', [Aaaaaaaaaaaaa, Bbbbbbbbbbbbbb]).fn2({prop: 'valueeee'});

	// Expression statement 101+ chars - short identifier (z) stays with first call
	// (isShort heuristic only applies to expression statements)
	z.fn1('type', [Aaaaaaaaaaaaaaaaaaaaa, Bbbbbbbbbbbbbbbbbbbbbb]).fn2({prop: 'valueeeeeeeeeeeeeeeeee'});
</script>
