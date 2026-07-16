<!-- regex literal containing `}` — closing-brace matching must skip over it -->
<div title="text1{f(/[}]/)}"></div>

<!-- the `}` is inside the second expression's regex, after a first expression -->
<div title="{a}{f(/[}]/)}"></div>

<!-- block comment containing `}` -->
<div title="text1{/* } */ b}"></div>

<!-- regex with balanced braces already matches; must stay correct -->
<div title="text1{f(/[{}]/g, b)}"></div>

<!-- block comment containing `{` — the unclosed brace must not deepen the scan -->
<div title="text1{/* { */ b}"></div>

<!-- block comment containing a quote or backtick — none of them opens a JS string,
	so the attribute's own closing quote is still found -->
<div title="text1{/* ' */ b}"></div>
<div title="text1{/* &quot; */ b}"></div>
<div title="text1{/* ` */ b}"></div>

<!-- line comment inside a multi-line expression, holding each delimiter -->
<div
	title="text1{f(
		// ` ' { }
		b
	)}"
></div>

<!-- regex containing the attribute's own quote, or one that would open a JS string -->
<div title="text1{f(/&quot;/)}"></div>
<div title="text1{f(/'/)}"></div>
<div title="text1{f(/`/)}"></div>

<!-- template literal interpolating an expression that holds each delimiter -->
<div title="text1{`${f(/[{}`'&quot;]/)}`}"></div>
