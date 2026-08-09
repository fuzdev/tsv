<script lang="ts">
	// Line comment between the enum header and the body `{` — `{` drops to the next line
	enum E { // c1
		A
	}

	// Empty body — the collapsed braces still drop below the comment
	enum F {} // c2

	// Multiple line comments, each kept on its own line (not merged)
	enum G // c4 // c3
	{
		B
	}

	// Line then block comment — the boundary is preserved, each on its own line
	enum H /* c6 */ // c5
	{}

	// `const enum` takes the same rule
	const enum K { // c7
		C
	}

	// A single-line block comment collapses onto the header line and `{` hugs it
	enum L /* c8 */ {
		D
	}
</script>
