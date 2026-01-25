<script lang="ts">
	// Arrow body is parenthesized ternary with chain containing nested arrow
	// Prettier removes parens and breaks after =>
	var f = (x) => (x ? x.aaa().map((s) => s.bbb()).filter(Boolean) : []);
</script>
