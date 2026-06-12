<script lang="ts">
	// Factory chain: uppercase-start IS a factory (break after =)
	const aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa =
		Util.fn('aaaaaaaaaa').optional();

	// $ prefix: NOT a factory (chain formatter handles it)
	const bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb = $util
		.fn('aaaaaaaaaa')
		.optional();

	// _ prefix: NOT a factory (chain formatter handles it)
	const ccccccccccccccccccccccccccccccccccccccccccccccccccccccccc = _helper
		.fn('aaaaaaaaaa')
		.optional();

	// Control: short lines (no breaking needed)
	const d = $util.fn('arg').optional();
	const e = _helper.fn('arg').optional();
	const f = Util.fn('arg').optional();
</script>
