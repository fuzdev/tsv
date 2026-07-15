<script lang="ts">
	// case A: later union member with leading line comment - relocates to trail the
	// previous member
	type A=a|(// leading
	b);

	// case B: paren retained (intersection in union)
	type B=a|(// leading
	b&c);

	// case C: conditional extends
	type C=a extends(// comment
	b)?c:d;

	// case M1: 3-member union, leading on middle
	type M1=a|(// leading
	b)|c;

	// case M2: 3-member union, leading on last
	type M2=a|b|(// leading
	c);
</script>
