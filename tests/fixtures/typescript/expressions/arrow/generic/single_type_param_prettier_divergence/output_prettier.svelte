<script lang="ts">
	// Single unconstrained type param stays bare (see README — prettier forces `<T,>`).
	const identity = <T,>(x: T) => x;

	// Default-only param (no constraint) is the same case — stays bare.
	const withDefault = <T = string,>(x: T) => x;

	let value = 1;
</script>

<!-- Template expression — the divergence applies in `{...}` too. -->
{(<T,>(x: T) => x)(value)}
