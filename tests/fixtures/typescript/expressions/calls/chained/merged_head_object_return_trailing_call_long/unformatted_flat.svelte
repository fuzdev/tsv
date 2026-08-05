<script lang="ts">
	function fn() {
		// 100 visual on the flat line - whole chain stays inline
		const a = Items.map((x) => ({ a: x.a, b: fn1(x.aaaaaaaaaaaaaaaaaaaaaaaaaaa) })).filter(Boolean);

		// 101 visual flat - the last call's args expand, the head stays merged and flat
		const b = Items.map((x) => ({ a: x.a, b: fn1(x.aaaaaaaaaaaaaaaaaaaaaaaaaaaa) })).filter(Boolean);

		// head segment through `.filter(` at exactly 100 - the last-call expansion still fits
		const c = Items.map((x) => ({ a: x.a, b: fn1(x.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa) })).filter(Boolean);

		// head segment through `.filter(` at 101 - the object hugs the callback parens instead
		const d = Items.map((x) => ({ a: x.a, b: fn1(x.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa) })).filter(Boolean);
	}
</script>
