<script lang="ts">
	// A tail holding a computed lookup or a `!` peels off the call the way a plain `.prop`
	// tail does: the call keeps its own group, each `.prop` keeps member.js's break point, and
	// a computed `[i]` / `?.[0]` or a `!` glues to whatever precedes it.

	// 100 - the call and the whole tail fit on one line
	const aaaa = bbbb.cccc(dddddddddddddddddd, dddddddddddddddddd)?.eeeeeeeeeeeeeeeeeeeeeeeeeeee?.[0];

	// 101 - the lookup drops with `?.[0]` glued to it; the call's arguments stay inline
	const aaaaa = bbbb.cccc(dddddddddddddddddd, dddddddddddddddddd)
		?.eeeeeeeeeeeeeeeeeeeeeeeeeeee?.[0];

	// a `!` and a numeric index glue to the call; the lookup after them is the break point
	bbbb.cccc(dddddddddddddddddd, dddddddddddddddddd, dddddddddddddddddd)![0]
		.eeeeeeeeeeeeeeeeeeeeeeee;

	// a non-numeric index breaks inside its own brackets before the lookup ahead of it does
	const ffffff = bbbb.cccc(dddddddddddddddddd, dddddddddddddddddd)?.eeeeeeeeeeeeeeeeeeeeeee?.[
		index
	];

	// a `!` after the lookup drops with it
	bbbb.cccc(dddddddddddddddddd, dddddddddddddddddd, dddddddddddddddddd)!
		.eeeeeeeeeeeeeeeeeeeeeeeeee!;
</script>
