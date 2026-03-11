<script lang="ts">
	/* signature at exactly 100 chars - params break, ternary body inline with parens */
	export const fn_aaaaaaaaaaaaaaaaaaaaaaaaaaa = (
		a: string,
		b: Set<string> | null,
	): string | null => b ? a : null;

	/* signature at 99 chars - params stay flat, ternary body on next line */
	export const fn_aaaaaaaaaaaaaaaaaaaaaaaaaa = (a: string, b: Set<string> | null): string | null =>
		(b ? a : null);

	/* signature at 101 chars - params break, ternary body inline with parens */
	export const fn_aaaaaaaaaaaaaaaaaaaaaaaaaaaa = (
		a: string,
		b: Set<string> | null,
	): string | null => b ? a : null;
</script>
