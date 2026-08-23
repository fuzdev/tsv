<script lang="ts">
	// An operator keyword cannot head a qualified type name, so `keyof.a` and
	// `readonly.a` are member accesses on ordinary value names and each `<` is
	// the less-than operator: a comparison, kept as written.
	const g1 = p < keyof.a > (t, u);
	const g2 = p < readonly.a > (t, u);
</script>
