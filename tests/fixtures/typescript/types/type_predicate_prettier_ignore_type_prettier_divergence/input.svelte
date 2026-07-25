<script lang="ts">
	// an own-line directive after `is` stays own-line and freezes the predicate type
	function fn(a: unknown): a is
		// prettier-ignore
		{x:   1} {
		return true;
	}
</script>
