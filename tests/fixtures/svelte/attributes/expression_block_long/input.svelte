<!-- Simple identifier expression -->

<!-- 100 chars - stays inline (at boundary) -->
<div data-x={aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa}></div>

<!-- 101 chars - wraps attributes -->
<div
	data-x={aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa}
></div>

<!-- Function call expression -->

<!-- 100 chars - stays inline (at boundary) -->
<div data-x={fn(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa)}></div>

<!-- 101 chars - wraps attributes -->
<div
	data-x={fn(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa)}
></div>

<!-- Function call with args that wrap internally -->
<div
	data-x={fn(
		arg0000000000000000,
		arg1111111111111111,
		arg2222222222222222,
		arg3333333333333333333333
	)}
></div>
