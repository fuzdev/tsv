<script lang="ts">
	// A for-in/for-of head binds exactly one declarator (`for ( ForDeclaration
	// in/of … )`, where `ForDeclaration` is a single `ForBinding`). Multiple
	// declarators is a syntax error (acorn `Unexpected token` at `in`/`of`). The
	// valid baselines below bind one; a C-style `for (;;)` may bind several.
	for (const x of [1]) {
		void x;
	}

	for (const k in { a: 1 }) {
		void k;
	}

	for (let i = 0, j = 1; i < j; i++) {
		void i;
	}
</script>
