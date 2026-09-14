<script lang="ts">
	type F = (...a: any[]) => void;
	type G = (b: string, ...a: any[]) => void;

	// A run the author GLUED — a multi-line block ahead of a single-line one — and broke
	// after, in the `...`→param gap: both formatters drop the parameter below the run
	type H = (
		.../* c1
	d1 */ /* c2 */
		c: any[]
	) => void;
</script>
