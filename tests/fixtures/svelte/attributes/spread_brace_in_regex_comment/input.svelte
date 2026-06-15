<!-- `}` inside a regex, string, or comment must not close the {...} early -->
<div {...f(/[}]/)}></div>
<div {...g('}')}></div>
<div {...h(/* } */ k)}></div>
