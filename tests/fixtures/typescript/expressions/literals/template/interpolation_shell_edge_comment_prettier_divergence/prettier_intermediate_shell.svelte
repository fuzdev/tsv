<script lang="ts">
	// A comment in a stripped shell at the value's HEAD prints right behind the `${`
	const aaa = `x${/* c1 */ bbb`
`}y`;
	const ccc = `x${/* c2 */ ddd // e2
		.then((v) => v)}y`;
	fff(
		`x${/* c3 */ ggg.each(function () {
			return 1;
		})}y`
	);
	const ooo = `w${`x${/* c6 */ ppp`
`}y`}z`;
	const qqq = `x${/* c7 */ rrr!`
`}y`;

	// ... and one in a stripped shell at the value's END prints at its end
	const hhh = `x${() =>
		iii.each(function () {
			return 1;
		}) + jjj /* c4 */}y`;
</script>

<p>
	{`x${/* c5 */ nnn`
`}y`}
</p>

<p
	class={`x${/* c8 */ sss`
`}y`}
>
	t
</p>
