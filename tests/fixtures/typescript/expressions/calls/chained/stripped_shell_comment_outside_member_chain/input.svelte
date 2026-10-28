<script lang="ts">
	// A comment in grouping parens the parser strips from around a member's object prints
	// mid-chain, in front of that member, only where the member is a link of a member chain a
	// call roots. With no call after the member, or a call whose callee is a `!`, it leads the
	// chain, ahead of every comment the object holds.
	/* c1 */ x /* c2 */().h[x];
	/* c3 */ a /* c4 */().q?.r;
	/* c5 */ x /* c6 */().h[x].k;
	/* c7 */ x /* c8 */().h[x]!;
	/* c9 */ x.a.t!();
	const v = /* c10 */ x /* c11 */().h[x];
	// the same with the shell's comment alone
	/* c14 */ x().h[x];
	/* c15 */ a().q?.r;
</script>

<p>{/* c12 */ x /* c13 */().h[x]}</p>
