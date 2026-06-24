<script lang="ts">
	// Single async generic arrow that stacks BOTH divergences on one construct:
	//  - Parser (Svelte): acorn-typescript drops the `x` param (expected_svelte vs expected_ours)
	//  - Formatter (Prettier): forces `<T,>` on the single unconstrained type param (output_prettier)
	const f = async <T>(x: T): Promise<T> => x;
</script>
