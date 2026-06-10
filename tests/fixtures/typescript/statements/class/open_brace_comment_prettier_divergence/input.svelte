<script lang="ts">
	// line comment trailing a class body `{`
	class C { // c1
		a = 1;
	}

	// block comment trailing the body `{`, then own-line leading comment
	class D { /* brace */
		// lead
		b = 2;
	}
</script>
