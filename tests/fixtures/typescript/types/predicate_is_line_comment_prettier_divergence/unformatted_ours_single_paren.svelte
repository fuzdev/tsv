<script lang="ts">
	// Type predicate: comment after `is`, predicate type on the next line
	function f(x): x is (// c
		T) {
		return true;
	}
</script>
