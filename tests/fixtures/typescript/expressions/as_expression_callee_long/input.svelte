<script lang="ts">
	// A cast in CALLEE position - of a call or a `new` - hangs its own parens: the operand
	// drops to an indented line and the `)` takes a line of its own, the same shape the cast
	// takes as a member-access object.

	// 100 - the callee fits, the parens stay welded
	(aaaaaaaaaa.bbbbbbbbbb.cccccccccc.dddddddddd.eeeeeeeeee.ffffffffff.gggggggggg.hhhhhhhhhh as Tt)();

	// 101 - one char longer: the parens hang
	(
		aaaaaaaaaaa.bbbbbbbbbb.cccccccccc.dddddddddd.eeeeeeeeee.ffffffffff.gggggggggg.hhhhhhhhhh as Tt
	)();

	// `satisfies` shares the cast path
	(
		aaaaaaaaaa.bbbbbbbbbb.cccccccccc.ddddddddd.eeeeeeeee.fffffffff.ggggggggg.hhhhhhhhh satisfies Tt
	)();

	// a `new` callee is the same position
	new (
		aaaaaaaaaa.bbbbbbbbbb.cccccccccc.dddddddddd.eeeeeeeeee.ffffffffff.gggggggggg.hhhhhhhhhh as Tt
	)();

	// the member-access object the rule already covers
	const qq = (
		aaaaaaaaaa.bbbbbbbbbb.cccccccccc.ddddddddd.eeeeeeeee.fffffffff.ggggggggg.hhhhhhhhh as Tt
	).hh;

	// NOT the rule: an ordinary value position keeps the plain cast layout
	const rr = aaaaaaaaaa.bbbbbbbbbb.cccccccccc.ddddddddd.eeeeeeeee.fffffffff.ggggggggg
		.hhhhhhhhh as Tt;
</script>
