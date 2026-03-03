<script lang="ts">
	function fn(a: number, b: any) {
		throw new Error(`${a}
multiline
etc ${a + b}
`);
	}
</script>
