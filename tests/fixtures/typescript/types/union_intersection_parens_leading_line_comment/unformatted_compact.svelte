<script lang="ts">
	// case A: a comment authored trailing a union member stays trailing in both
	// formatters (contrast a comment authored inside a LATER member's parens, which
	// tsv keeps inside — see union_intersection_retained_paren_leading_line_comment)
	type A=a// leading
	|b;

	// case B: paren retained (intersection in union)
	type B=a// leading
	|(b&c);

	// case C: conditional extends
	type C=a extends b// comment
	?c:d;

	// case M1: 3-member union, leading on middle
	type M1=a// leading
	|b|c;

	// case M2: 3-member union, leading on last
	type M2=a|b// leading
	|c;
</script>
