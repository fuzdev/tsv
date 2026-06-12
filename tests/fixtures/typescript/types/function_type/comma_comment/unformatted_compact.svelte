<script lang="ts">
	// Inline block comment after comma (leading on next param)
	type A = (a: string,/* c */b: number) => void;

	// Inline block comment before comma (trailing on current param)
	type B = (a: string/* c */,b: number) => void;

	// Both trailing and leading comments around comma
	type C = (a: string/* c1 */,/* c2 */b: number) => void;
</script>
