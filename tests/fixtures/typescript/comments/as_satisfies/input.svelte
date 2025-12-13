<script lang="ts">
	// Multiline comment after as const
	1 as const /*
comment
*/;

	// Comment between value and as
	1 /* comment */ as const;

	// Satisfies with comment
	x /* comment */ satisfies T;
</script>
