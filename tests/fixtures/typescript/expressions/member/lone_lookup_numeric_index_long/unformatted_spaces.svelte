<script lang="ts">
	// Prettier's `shouldInline` lone-`.prop` clause needs the lookup's PARENT not to be a
	// member — and a computed lookup is one, so `a.p[0]` gives the `.p` a break point
	// exactly as `a.p.q` does. The index then rides the line the lookup lands on, sitting
	// OUTSIDE the lookup's own group.

	class Q {
		m(objx :  Q ,  iii :  number)  {
			// a numeric index keeps its brackets flat, so the lookup is the only break point -
			// at exactly 100 the whole thing still fits
			objx . aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa[ 0 ];

			// one char more, and the lookup takes its own line, carrying the index with it
			objx
				. aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa[ 0 ];

			// the `!` and `?.` spellings answer the same way
			objx
				. aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa! [ 0 ];
			objx
				. aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa ?. [ 0 ];

			// a NON-numeric index has breakable brackets, and THEY shed the width instead: the
			// lookup's group is measured only as far as the bracket softline, so it stays flat
			objx . aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa[
				iii
			];
		}
	}
</script>
