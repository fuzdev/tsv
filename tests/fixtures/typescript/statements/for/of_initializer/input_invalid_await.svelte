<script lang="ts">
	async function fn() {
		for await (var x = 1 of []) {
		}
	}
</script>
