<script lang="ts">
	// a single-member union collapses (drops its `|`) when reformatted, so a member-only
	// freeze is non-idempotent — pass 2 would see a bare member no longer routed through the
	// union. tsv keeps the `|` under the freeze (idempotent); prettier drops it
	type A =
		// prettier-ignore
		| {a:1};

	// leaf sole member: same keep-the-pipe rule
	type B =
		// prettier-ignore
		| foo;

	// a COMPOSITE sole member is transparent for directive binding: tsv collapses and lets
	// the inner intersection apply Rule A (freezing its first member), so the tsv-stable form
	// is the bare `a1 & a2` — the intersection-first-member behavior, no pipe kept
	type C =
		// prettier-ignore
		a1 & a2;
</script>
