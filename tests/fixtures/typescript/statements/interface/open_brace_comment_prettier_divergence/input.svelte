<script lang="ts">
	// line comment trailing an interface body `{`
	interface I { // c1
		a: number;
	}

	// block comment trailing the body `{`, then own-line leading comment
	interface J { /* brace */
		// lead
		b: string;
	}
</script>
