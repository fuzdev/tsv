<script lang="ts">
	async function* fn() {
		await (yield x);
	}
	async function fn2() {
		await (() => {});
	}
</script>
