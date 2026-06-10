<script lang="ts">
	// 中文 😀 shift byte offsets past UTF-16 offsets — the type-param trailing
	// comma position (extra.trailingComma) below must translate too
	const fn = <T,>(x: T) => x;
	const withDefault = <U = string,>(y: U) => y;
</script>
