<script lang="ts">
	// line comment trailing a namespace body `{` - single statement
	namespace N1 {
		// c1
		const a = 1;
	}

	// line comment trailing a namespace body `{` - multiple statements
	namespace N2 {
		// c2
		const b = 2;
		const c = 3;
	}

	// block comment trailing the body `{`, then own-line leading comment
	namespace N3 {
		/* brace */
		// lead
		const d = 4;
	}

	// line comment trailing a `module` body `{`
	module M1 {
		// c3
		const e = 5;
	}
</script>
