<script>
	/* 100 chars total - array stays inline */
	const a = [aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa && bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbabcdefghijklmnopqrst];

	/* 101 chars total - array wraps, binary stays inline */
	const b = [
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa && bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbabcdefghijklmnopqrstu
	];

	/* 100 chars element line - binary stays inline */
	const c = [
		aaaaaaaaaaaaaaaaaaaaaaaaaaa && bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbabcdefg
	];

	/* 101 chars element line - binary breaks, continuation gets extra indent */
	const d = [
		aaaaaaaaaaaaaaaaaaaaaaaaaaa &&
			bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbabcdefgh
	];

	/* Multi-operand chain - each continuation indented */
	const e = [
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa &&
			bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb &&
			cccccccccccccccccccccccccccccc
	];
</script>
