<script lang="ts">
	// Inline block comment after comma (leading on next member)
	enum A {
		X,
		/* c */ Y
	}

	// Inline block comment before comma (trailing on current member)
	enum B {
		X /* c */,
		Y
	}

	// Both trailing and leading comments around comma
	enum C {
		X /* c1 */,
		/* c2 */ Y
	}
</script>
