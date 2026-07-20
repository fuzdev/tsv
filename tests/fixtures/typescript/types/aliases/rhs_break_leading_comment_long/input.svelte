<script lang="ts">
	// A glued block comment after `=` leads the RHS, so where the RHS goes, the
	// comment goes: it stays on the `=` line while the RHS head hugs there, and
	// rides down onto its own line when the whole RHS relocates below `=`.

	// Fits at 100 columns (effective): comment and the whole union stay on the `=` line.
	type Nxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx = /* c */ 'firstMember' | 'secondMember' | 'thirdMember';

	// One column longer: the union relocates below `=` and the comment rides with it.
	type Nxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx =
		/* c */ 'firstMember' | 'secondMember' | 'thirdMember';

	// A bare reference RHS that overflows relocates below `=`; the comment rides down.
	type Rxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx =
		/* c */ SomeVeryLongTypeReferenceNameThatForcesTheRhsBelowTheEqualsSign;

	// A conditional whose check/extends head overflows the `=` line relocates entirely
	// below `=` (head and both branches), and the comment rides down with it.
	type Cxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx =
		/* c */ Param extends Ccccccccccccccccccccccccccccccc ? Tnnnnnnnnn : Fnnnnnnnnn;

	// Contrast: a conditional whose head fits the `=` line keeps the comment there
	// and breaks only at `?`/`:`.
	type Hxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx = /* c */ Check extends Extendsssssssssss
		? Trueeeeeeeeeeebranch
		: Falseeeeeeeeeebranch;

	// Contrast: an intersection whose first member hugs the `=` line keeps the comment
	// there and breaks between members.
	type Ixxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx = /* c */ Mmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmm &
		Nnnnnnnnnnnnnnnnnn &
		Ooooooooooo;
</script>
