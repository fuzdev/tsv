<script lang="ts">
	// a glued directive is an ordinary comment: the clause formats normally
	for (/* prettier-ignore */ i = 0; i < 10; i++) {
		fn();
	}

	function fn1() {
		// the same at a restricted production's grouping `(`
		return /* prettier-ignore */ a + b;
	}

	// and at a condition head's `(`
	if (/* prettier-ignore */ a + b) {
		fn();
	}

	// and at an assignment operator, where the value is likewise reformatted
	const c = /* prettier-ignore */ d + e;

	const obj = { f: /* prettier-ignore */ g + h };

	// and at the value heads whose delimiter is an operator, a `${` or a `[`
	const iii = typeof (/* prettier-ignore */ jjj ?? kkk);

	const lll = `x${/* prettier-ignore */ mmm + nnn}y`;

	const ooo = { [/* prettier-ignore */ ppp + qqq]: rrr };

	// A directive the author TRAILED on an opening delimiter's line is inert for the same
	// reason, and tsv keeps it on that line rather than forcing it onto its own — moving it
	// would silently ARM a freeze the author's placement never asked for.
	const sss = {
		[ // prettier-ignore
			ttt + uuu
		]: vvv
	};

	fn1(
		...// prettier-ignore
		(www ?? xxx)
	);
</script>
