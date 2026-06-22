<script>
	// update/unary/arrow operands need parens before member access (body absorbs `.p` otherwise)
	const a = (++n).p;
	const b = (n--).p;
	const c = (-n).p;
	const d = (!x).p;
	const e = (typeof x).p;
	const f = (void x).p;
	const g = (delete x.y).p;
	const h = (() => 1).p;
	// optional + computed member forms
	const i = (++n)?.p;
	const j = (++n)[0];
	// brace-delimited bodies are unambiguous — these strip
	const k = function () {}.p;
	const l = class {}.p;
	const m = { a: 1 }.p;
</script>
