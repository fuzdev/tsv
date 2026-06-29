<script lang="ts">
	function f(x: unknown): x // c
		is string {
		return typeof x === 'string';
	}
</script>
