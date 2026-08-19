<script lang="ts">
	// A block the author GLUED to the key leads it inline and the bracket stays flat — a
	// computed key never breaks on width.
	const aaa = { [/* c */ bbb + ccc]: ddd };

	// A block the author gave its OWN LINE keeps that line, so the bracket opens around it.
	const eee = {
		[
			/* c */
			fff + ggg
		]: hhh
	};

	// The same at a class member and at a destructuring pattern.
	class Iii {
		[
			/* c */
			jjj + kkk
		] = lll;
	}

	const {
		[
			/* c */
			mmm + nnn
		]: ooo
	} = ppp;

	// A run the author broke keeps each line.
	const qqq = {
		[
			/* c1 */
			/* c2 */
			rrr + sss
		]: ttt
	};
</script>
