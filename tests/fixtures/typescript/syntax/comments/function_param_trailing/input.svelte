<script lang="ts">
	const fn = (
		a: string,
		b: unknown, // line comment
		c?: number,
	): void => {};

	const fn2 = (a: string, b: unknown /* block comment */, c?: number): void => {};
</script>
