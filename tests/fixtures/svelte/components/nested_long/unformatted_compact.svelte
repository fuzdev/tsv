<!-- Component at indent 1: 100 effective (no wrap) -->
<Outer>
	<Inner prop="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" />
</Outer>

<!-- Component at indent 1: 101 effective (WRAPS) -->
<Outer>
	<Inner prop="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" />
</Outer>

<!-- Component at indent 2: 100 effective (no wrap) -->
<Level1>
	<Level2>
		<Inner prop="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" />
	</Level2>
</Level1>

<!-- Component at indent 2: 101 effective (WRAPS) -->
<Level1>
	<Level2>
		<Inner prop="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" />
	</Level2>
</Level1>
