<style>
	/* a hex escape that decodes to a backslash, at each name site */
	#a\5c b {
		color: red;
	}
	.a\5c b {
		color: red;
	}
	a\5c b {
		color: red;
	}
	:a\5c b {
		color: red;
	}
	::a\5c b {
		color: red;
	}
	[a\5c b] {
		color: red;
	}
	a\5c b|c {
		color: red;
	}
	/* every hex spelling of the backslash: uppercase, zero-padded, and a terminator
	   before the block */
	.a\5C b,
	.a\00005c b,
	.a\5c {
		color: blue;
	}
	/* contrast: an identity-escaped backslash, and a hex escape of another character */
	.a\\b,
	.a\41 b {
		color: green;
	}
</style>
