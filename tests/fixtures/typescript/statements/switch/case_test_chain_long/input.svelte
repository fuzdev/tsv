<script>
	switch (x) {
		// A case test's chain takes a continuation indent - a SwitchCase parent is in
		// neither of prettier's exempt lists, so the chain arm fires
		case bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ??
			cccccccccccccccccccccccccccccccccccccccccccccccccc ??
			ddd:
			break;

		// 100 chars - stays inline
		case bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ?? ccccccccccccccccccccccccccccccccccccccc ?? ddd:
			break;

		// 101 chars - the chain breaks
		case bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ??
			cccccccccccccccccccccccccccccccccccccccc ??
			ddd:
			break;
	}
</script>
