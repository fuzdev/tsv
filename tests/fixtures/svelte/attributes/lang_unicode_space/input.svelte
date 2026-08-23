<script lang="ts">
	const x = 1;
</script>

<!--
	The `lang` the PRINTER reads is trimmed with a class WIDER than either of the crate's
	own - JS `\s` UNION Rust's `White_Space` - because tsv's formattable list is an
	allowlist that the trim feeds, so a wider class can only ever route a name toward
	FORMATTING. Either class alone gives one witness away and freezes a body prettier
	formats: JS `\s` keeps a U+0085 NEL (the `<script>` above) and `str::trim` keeps a
	U+FEFF (the `<style>` below), so both sit here and either single-class spelling fails
	one of them. Only unformatted_lang_unicode_space can tell a freeze from a format - an
	already-formatted body reads the same either way. The trim is tsv's own; prettier does
	not trim, it just fails to match its five-name denylist, so the two agree here by two
	different routes and the trim's own divergence is `elements/template_lang_trim`.
	Sibling questions: `attributes/lang_entity` (the raw wire reader vs this decoded one)
	and `attributes/lang_priority`.
-->

<style lang="﻿css">
	div {
		color: red;
	}
</style>
