<!-- Binary + continuation indent in template expressions -->
<!-- When the + continuation breaks, it gets extra indent -->

<!-- Short - stays inline -->
<span>
	{(a === bbb ? c : d) + (e ? f : g) || h}
</span>

<!-- 100 chars total - stays inline -->
<span>
	{(a === bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ? c : d) + (e ? f : g) || h}
</span>

<!-- 101 chars total - || h breaks to next line, + stays inline -->
<span>
	{(a === bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ? c : d) + (e ? f : g) ||
		h}
</span>

<!-- + continuation breaks, first line 100 chars -->
<span>
	{(a === bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ? c : d) +
		(e ? f : g) || h}
</span>

<!-- + continuation breaks, first line 101 chars - ternary breaks instead -->
<span>
	{(a === bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
		? c
		: d) + (e ? f : g) || h}
</span>
