<script lang="ts">
	async function test() {
		const a = (await x)(y);
		const b = (await x)`template`;
	}
	function* gen() {
		const c = (yield x)(y);
		const d = (yield x)`template`;
	}
</script>
