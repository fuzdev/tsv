<script lang="ts">
	// Integral doubles above u64::MAX (18446744073709551615) but below 1e21 expand
	// to their full digits under JS Number::toString, matching JSON.stringify.
	const a = 1e20;
	const b = 1.234e20;
	const c = -5e20;

	// At/above 1e21 JS switches to exponential form.
	const d = 1e21;

	// Just under u64::MAX still emits through the u64 branch (regression guard).
	const e = 18000000000000000000;

	// A non-integral value in [1e-6, 1e-5) stays fixed under JS, not scientific.
	const f = 0.000001;
</script>
