<script lang="ts">
	function f(x: unknown): x
		is string {
		return typeof x === 'string';
	}
</script>
