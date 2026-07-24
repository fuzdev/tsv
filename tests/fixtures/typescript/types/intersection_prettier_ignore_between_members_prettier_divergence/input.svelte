<script lang="ts">
	// an own-line directive between intersection members: tsv freezes only the following
	// member and keeps the directive own-line; prettier relocates it (trailing the preceding
	// & on pass 1) and its fixed point collapses the intersection inline, floating the
	// directive to the statement end and LOSING the freeze
	type I = { a: 1 } &
	// prettier-ignore
	{b:2} & { c: 3 };
</script>
