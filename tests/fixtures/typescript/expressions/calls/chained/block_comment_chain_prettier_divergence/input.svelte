<script lang="ts">
	// Double block comment - inner comment creates mid-chain comment
	function fn1(a) {
		/* outer */ a /* inner */.b
			.c(a);
	}

	// Double block comment on deeper chain member
	function fn2(a) {
		/* outer */ a.b /* inner */.c
			.d(a);
	}
</script>
