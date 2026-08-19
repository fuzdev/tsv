<script lang="ts">
	// A comment the author GLUED to what follows leads it inline, and the `${…}` stays on
	// one line.
	const aaa = `x${/* c */ bbb + ccc}y`;

	// A comment the author gave its OWN line keeps that line, and the interpolation opens
	// around it. Both authorings are fixed points — the placement is the author's.
	const ddd = `x${
		/* c */
		eee + fff
	}y`;

	// A `//` always breaks: inline it would swallow the rest of the interpolation.
	const ggg = `x${
		// c
		hhh + iii
	}y`;

	// A run the author glued together stays glued.
	const jjj = `x${/* c1 */ /* c2 */ kkk + lll}y`;

	// A run the author broke keeps its two lines.
	const mmm = `x${
		/* c1 */
		/* c2 */
		nnn + ooo
	}y`;

	// A trailing block hugs the value; the interpolation stays flat.
	const ppp = `x${qqq + rrr /* c */}y`;

	// A trailing `//` breaks the interpolation so it cannot swallow the `}`.
	const sss = `x${
		ttt + uuu // c
	}y`;
</script>
