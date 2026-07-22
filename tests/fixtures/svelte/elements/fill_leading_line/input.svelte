<!-- fits: the leading `line` before `ccccc` renders flat, as a space -->
<li>
	aaaaa ~{xxxxxxxxxx} bbbbb ~{yyyyyyyyyy} ccccc ccccc ccccc
</li>
<!-- overflows: the glued `~{yyyyyyyyyy}` rides its line (prettier keeps it, tsv now matches), and
     the leading `line` before `ccccc` becomes the newline itself — not a newline followed by the
     space it stood for, which would strand a leading space at the head of the continuation line -->
<li>
	aaaaa aaaaa aaaaa aaaaa aaaaa ~{xxxxxxxxxx} bbbbb bbbbb bbbbb bbbbb bbbbb bbbbb bbbbb ~{yyyyyyyyyy}
	ccccc ccccc ccccc ccccc
</li>
