<script lang="ts">
	// an own-line directive in an enum member's `=`→value gap freezes the whole value
	enum Aaa {
		Bbb =
			// prettier-ignore
			ccc  +  ddd
	}

	// an own-line block comment behaves identically — placement keys the freeze, not the spelling
	enum Eee {
		Fff =
			/* prettier-ignore */
			ggg  +  hhh
	}

	// a `const enum` member is the same host, and a sibling member the freeze does not reach
	// still normalizes
	const enum Iii {
		Jjj =
			// prettier-ignore
			kkk  +  lll,
		Mmm = nnn + ooo
	}

	// the rule is not the directive's — any own-line comment after the `=` leads the value from
	// its own line here
	enum Ppp {
		Qqq =
			// c
			rrr + sss
	}

	// the member loop's own trailing scan claims the gap between the frozen slice and a
	// stripped paren, so a comment written there needs no shell of the value's own
	enum Ttt {
		Uuu =
			// prettier-ignore
			vvv  +  www /* c */
	}
</script>
