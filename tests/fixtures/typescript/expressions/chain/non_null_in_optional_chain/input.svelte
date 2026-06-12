<script lang="ts">
	// a parenthesized optional chain sealed by `!` and then continued — the inner
	// chain closes at the parens, the outer subscripts form their own chain
	a?.b()!.c?.();
	a?.b()!?.();
	(a?.b())!.c;

	// `!` inside the parens — same sealing, the NonNull rides inside the inner chain
	(a?.b!.c).d;
	(a?.b!()).c?.();
</script>
