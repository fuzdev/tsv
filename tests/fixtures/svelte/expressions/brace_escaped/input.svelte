<div>
	<!-- Single -->
	{'{'}
	{'}'}

	<!-- With trailing text -->
	{'{'}text}

	<!-- With text between -->
	{'{'}text{'}'}

	<!-- Adjacent (no text) -->
	{'{'}{'}'}

	<!-- Multiple pairs -->
	{'{'}a}{'{'}b}

	<!-- Adjacent to expression -->
	{'{'}} {expr}

	<!-- In inline element -->
	<code>{'{'}value}</code>

	<!-- In attribute -->
	<span title="{'{'}value}">text</span>
</div>
