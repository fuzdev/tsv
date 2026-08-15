<script lang="ts">
	// Declarator - a `//` trailing the `=` above an owned MULTILINE block
	const a = /* owned // c1

		*/ 1;

	// Declarator - the same with a single-line block; only the block's own line
	// count differs (null control)
	const b = /* owned */ 1; // c1

	// Assignment - the same gap
	target =
		// c1
		/* owned

		*/ 1;

	// Class property
	class C {
		p = /* owned // c1

			*/ 1;
	}

	// Enum member
	enum E {
		A = /* owned // c1

			*/ 1
	}

	// Arrow body - the same authoring, which prettier leaves alone (control)
	const c = () =>
		// c1
		/* owned

		*/ 1;
</script>
