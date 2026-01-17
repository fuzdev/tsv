<script lang="ts">
	// 100 chars - stays flat (both formatters match)
	const a = `xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx ${expr} yyyyyyyy`;

	// 101 chars - we break, prettier stays inline
	const b = `xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx ${
		expr
	} yyyyyyyy`;
</script>
