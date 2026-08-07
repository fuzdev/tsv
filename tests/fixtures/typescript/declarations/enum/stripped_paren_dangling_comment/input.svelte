<script lang="ts">
	// a block comment past the last member takes its own line before the closer
	enum E1 {
		A = 1
		/* c */
	}

	// a line comment in the same position
	enum E2 {
		A = 1
		// c
	}
</script>
