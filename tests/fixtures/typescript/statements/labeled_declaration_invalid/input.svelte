<script lang="ts">
	// `LabelledItem : Statement | FunctionDeclaration`; a lexical declaration
	// (`let`/`const`), a class, or (in strict/module code) a function declaration
	// is not a labelable statement (acorn `Unexpected token`). A `var` statement,
	// a block, and ordinary statements stay valid.
	outer: for (const x of [1]) {
		void x;
	}

	v: var n = 1;

	b: {
		void n;
	}
</script>
