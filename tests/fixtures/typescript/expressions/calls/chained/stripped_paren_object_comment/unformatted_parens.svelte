<script lang="ts">
	// A stripped grouping paren around a chain head does not re-print the head's own comments
	function fn1(a) {
		((a /* c1 */).b.c).d(a);
	}

	function fn2(a) {
		((a /* c2 */).b.c.d).e(a);
	}
</script>
