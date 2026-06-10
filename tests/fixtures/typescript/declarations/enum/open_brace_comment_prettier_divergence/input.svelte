<script lang="ts">
	// line comment trailing an enum body `{`
	enum E { // c1
		A,
	}

	// block comment trailing the body `{`, then own-line leading comment
	enum F { /* brace */
		// lead
		B,
	}
</script>
