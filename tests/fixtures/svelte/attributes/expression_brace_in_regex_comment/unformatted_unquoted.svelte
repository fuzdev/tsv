<!-- regex literal containing `}` — closing-brace matching must skip over it -->
<div title=text1{f(/[}]/)}></div>

<!-- the `}` is inside the second expression's regex, after a first expression -->
<div title={a}{f(/[}]/)}></div>

<!-- block comment containing `}` -->
<div title=text1{/* } */ b}></div>

<!-- regex with balanced braces already matches; must stay correct -->
<div title=text1{f(/[{}]/g, b)}></div>
