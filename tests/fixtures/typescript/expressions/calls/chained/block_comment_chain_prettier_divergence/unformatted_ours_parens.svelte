<script lang="ts">
	// Double block comment - inner comment creates mid-chain comment
	function fn1(a) {
		/* outer */ (/* inner */ (a).b).c(a);
	}

	// Double block comment on deeper chain member
	function fn2(a) {
		/* outer */ (/* inner */ (a).b.c).d(a);
	}
</script>
