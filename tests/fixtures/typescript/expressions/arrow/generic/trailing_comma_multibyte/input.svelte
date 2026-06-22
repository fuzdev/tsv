<script lang="ts">
	// 中文 😀 shift byte offsets past UTF-16 offsets — the trailing comma both
	// formatters keep on each breaking type-param list below (extra.trailingComma)
	// must translate too.
	const fn = <
		Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa extends Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
	>(
		x: Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
	) => x;

	const fn2 = <
		Cccccccccccccccccccccccccccccccccccccc extends Dddddddddddddddddddddddddddddddddddddddddddd
	>(
		y: Cccccccccccccccccccccccccccccccccccccc
	) => y;
</script>
