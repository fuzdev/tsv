<script lang="ts">
	// await/yield as chain base
	async function fn() {
		const a = (await x).prop;
		const b = (await x)[0];
		const c = (await x)?.prop;
		const d = (await x)!;
		const e = (await x)!.prop;
		const f = (await fn)<T>();
	}

	function* gen() {
		const a = (yield x).prop;
		const b = (yield x)[0];
		const c = (yield x)?.prop;
		const d = (yield x)!;
		const e = (yield x)!.prop;
		const f = (yield fn)<T>();
	}
</script>
