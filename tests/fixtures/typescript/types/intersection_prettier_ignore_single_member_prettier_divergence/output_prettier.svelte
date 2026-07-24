<script lang="ts">
	// a single-member intersection collapses (drops its `&`) when reformatted, so a
	// member-only freeze is non-idempotent — pass 2 would see a bare member no longer routed
	// through the intersection. tsv keeps the `&` under the freeze (idempotent); prettier
	// drops it — the union keep-the-pipe rule, mirrored
	type A =
		// prettier-ignore
		{a:1};

	// leaf sole member: same keep-the-ampersand rule
	type B =
		// prettier-ignore
		foo;

	// a COMPOSITE sole member is transparent for directive binding: tsv collapses and lets
	// the inner union apply Rule A (freezing its first member), so the tsv-stable form is
	// the bare `a1 | a2` — the union-first-member behavior, no ampersand kept
	type C =
		// prettier-ignore
		a1 | a2;
</script>
