<script>
	/* assignment 100 chars - stays inline */
	const a='aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'+'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb';

	/* assignment 101 chars - breaks at + */
	const b='aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'+'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb';

	/* call arg 100 chars - stays inline */
	fn('aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'+'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb1');

	/* call arg 101 chars - breaks at + */
	fn('aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'+'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb12');

	/* template 100 chars - stays inline */
	const c=`aaa ${expr} aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa`+`bbb ${expr} bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb`;

	/* template 101 chars - breaks at + */
	const d=`aaa ${expr} aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa`+`bbb ${expr} bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb`;
</script>
