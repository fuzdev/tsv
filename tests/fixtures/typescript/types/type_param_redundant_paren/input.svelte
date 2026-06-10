<script lang="ts">
	// Short union constraint: stripped, inline
	type ShortConstraintU<TParam extends A | B> = TParam;

	// Long union constraint: stripped, breaks after `extends`
	type LongConstraintU<
		TParam extends
			| Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
			| Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
			| Ccccccccccccccccccccccccccccccccccccccccccc,
	> = TParam;

	// Long intersection constraint: stripped, first member hugs, rest wrap
	type LongConstraintI<
		TParam extends Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa &
			Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb &
			Ccccccccccccccccccccccccccccccccccccccccccc,
	> = TParam;

	// Long union default: stripped, breaks after `=`
	type LongDefaultU<
		TParam =
			| Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
			| Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
			| Ccccccccccccccccccccccccccccccccccccccccccc,
	> = TParam;

	// Long intersection default: stripped, first member hugs, rest wrap
	type LongDefaultI<
		TParam = Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa &
			Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb &
			Ccccccccccccccccccccccccccccccccccccccccccc,
	> = TParam;
</script>
