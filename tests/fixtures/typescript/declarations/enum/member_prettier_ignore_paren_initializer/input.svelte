<script lang="ts">
	// prettier-ignore keeps the member verbatim through its closing paren
	enum E1 {
		// prettier-ignore
		A = (a,   b),
		B = 1
	}

	// the last member's closing paren is likewise inside the frozen slice
	enum E2 {
		A = 1,
		// prettier-ignore
		B = (a,   b)
	}
</script>
