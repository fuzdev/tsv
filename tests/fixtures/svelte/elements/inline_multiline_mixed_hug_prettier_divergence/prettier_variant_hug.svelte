<!-- Component + expression on separate lines (hug end) -->
<span>
	<Comp />
	{expr}</span
>

<!-- Expression + component on separate lines (hug end) -->
<span>
	{expr1}
	<Comp /></span
>

<!-- Element + expression on separate lines (hug end) -->
<span>
	<strong>text</strong>
	{expr}</span
>

<!-- Two components on separate lines (hug end) -->
<span>
	<Comp1 />
	<Comp2 /></span
>

<!-- With attributes (hug end) -->
<a href="x">
	<Comp />
	{expr}</a
>

<!-- Trailing whitespace (non-hug) - already works, control case -->
<span>
	<Comp />
	{expr}
</span>
