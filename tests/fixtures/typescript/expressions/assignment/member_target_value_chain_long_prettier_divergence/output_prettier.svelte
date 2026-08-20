<script>
	// The target rule stops at the target. Prettier's `shouldInline` walk (member.js) is
	// position-blind — it inlines every lookup in the whole assignment once the target is
	// not a plain identifier — so prettier drops the break points from the VALUE's chain
	// too and lets it overflow. tsv scopes the rule to the target and keeps the value
	// width-driven.

	// value chain at exactly 100 on its own line — both formatters keep it intact
	zz.yy =
		aaaaaaaaaaaa.bbbbbbbbbbbb.cccccccccccc.dddddddddddd.eeeeeeeeeeee.ffffffffffff.gggggggggggg.hhhh;

	// value chain at 101 — tsv breaks at the last lookup, prettier overflows
	zz.yy =
		aaaaaaaaaaaa.bbbbbbbbbbbb.cccccccccccc.dddddddddddd.eeeeeeeeeeee.ffffffffffff.gggggggggggg.hhhhh;

	// Control: an identifier target keeps prettier's walk out of it, so both formatters
	// break the same 101-char value chain
	zzyy =
		aaaaaaaaaaaa.bbbbbbbbbbbb.cccccccccccc.dddddddddddd.eeeeeeeeeeee.ffffffffffff.gggggggggggg
			.hhhhh;
</script>
