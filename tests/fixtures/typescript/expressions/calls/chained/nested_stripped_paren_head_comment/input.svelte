<script lang="ts">
	// A comment inside NESTED stripped grouping parens at a chain's head. Each
	// paren's own prefix belongs to the member whose gap linearization widened back
	// over it; everything from the innermost paren inward is the chain head's, and
	// the two claims must partition the region — with a single paren the head takes
	// all of it, and with two the region between them was claimed by neither.

	// c1
	a.b;

	// c2
	a.b.c;

	// c3
	a.b(x);

	// The head's share sits in the widened node's SKIPPED middle, so every layout
	// question asked about that gap has to skip it too — read whole, the range
	// force-expanded the chain around a comment printed above it
	// c5
	a.b.c(x);

	// The skipped middle is the object's WHOLE span, so a comment inside an inner
	// call's arguments is in it as well
	f(/* c6 */ 1).a.b.c(y);

	// a single paren — the head's whole region, the control
	// c4
	a.b(x);
</script>
