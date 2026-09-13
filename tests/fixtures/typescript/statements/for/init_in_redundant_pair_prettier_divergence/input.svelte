<script lang="ts">
	// A unary operator's own comment-holder shell already parenthesizes the `in` that a `for`
	// header's ambient `[~In]` rule asks for, so the operand takes ONE pair — the shell's.
	for (!(/* c */ 'aaa' in bbb); ;) {}

	for (typeof (/* c */ 'aaa' in bbb); ;) {}

	for (void (/* c */ 'aaa' in bbb); ;) {}

	// A call argument is `[+In]` in its own right, so a sequence written inside one needs no
	// pair around either operand.
	for (fn((lll, 'aaa' in bbb)); ;) {}

	for (fn(('aaa' in bbb, lll)); ;) {}

	// Controls. Without the comment the unary operand needs no shell and both tools print the
	// one pair, and a clause that IS the `in` takes that pair at its own position — bare, the
	// header would read as a for-in head and not parse at all.
	for (!('aaa' in bbb); ;) {}

	for (('aaa' in bbb); ;) {}
</script>
