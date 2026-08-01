<!-- The trailing comment counts toward the width: at exactly 100 the tag stays inline -->
<div data-a="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" /* ccccccccccccccccccccccccccccc */></div>

<!-- At 101 the open tag breaks, the comment staying trailing its attribute -->
<div
	data-a="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" /* ccccccccccccccccccccccccccccc */
></div>
