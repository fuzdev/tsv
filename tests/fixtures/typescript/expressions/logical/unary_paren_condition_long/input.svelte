<script lang="ts">
	// Short — !(logical) fits inline, no break
	if (!(aa.bb || cc.dd)) {
		x;
	}

	// 100 boundary — the whole `if (!(...)) {` line is exactly 100, stays inline
	if (!(aaaaaaaaaaaaaaaaaaaa.bbbbbbbbbb || cccccccccccccccccccc.dddddddddd || eeeeeeee.fffffffff)) {
		x;
	}

	// Over 100 — `if (!(` hugs, each operand on its own line (prettier shouldInlineCondition)
	if (!(
		aaaaaaaaaaaaaaaaaaaa.bbbbbbbbbb ||
		cccccccccccccccccccc.dddddddddd ||
		eeeeeeee.ffffffffff
	)) {
		x;
	}

	// Double-not — `!!(logical)` also hugs
	if (!!(
		aaaaaaaaaaaaaaaaaaaa.bbbbbbbbbb ||
		cccccccccccccccccccc.dddddddddd ||
		eeeeeeeeee.ffffffffff
	)) {
		x;
	}

	// Nullish — `??` is a logical operator, so `!(a ?? b)` hugs too
	if (!(
		aaaaaaaaaaaaaaaaaaaa.bbbbbbbbbb ??
		cccccccccccccccccccc.dddddddddd ??
		eeeeeeeeee.ffffffffff
	)) {
		x;
	}

	// while — same hug as if
	while (!(
		aaaaaaaaaaaaaaaaaaaa.bbbbbbbbbb ||
		cccccccccccccccccccc.dddddddddd ||
		eeeeeeeeeeee.ffffff
	)) {
		x;
	}

	// Contrast — a bare logical condition is NOT inlined: it breaks onto its own line
	if (
		aaaaaaaaaaaaaaaaaaaa.bbbbbbbbbb ||
		cccccccccccccccccccc.dddddddddd ||
		eeeeeeeeee.ffffffffffff
	) {
		x;
	}

	// Contrast — `!!!` (triple-not) is NOT inlined: only `!` and `!!` of a logical qualify
	if (
		!!!(aaaaaaaaaaaaaaaaaaaa.bbbbbbbbbb || cccccccccccccccccccc.dddddddddd || eeeeeeee.ffffffff)
	) {
		x;
	}
</script>
