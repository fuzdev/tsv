<style>
	/* concatenated hex escapes: \6c\74\72 decodes to ltr */
	p:dir(\6c\74\72) {
		color: red;
	}

	/* hex escape + space terminator + literal: \65 n decodes to en */
	span:lang(\65 n) {
		font-family: serif;
	}

	/* uppercase hex digits: \65\6E decodes to en */
	span:lang(\65\6E) {
		font-weight: bold;
	}

	/* literal interleaved with an escape: l\74r decodes to ltr */
	p:dir(l\74r) {
		text-align: left;
	}

	/* identity escape (non-hex) keeps the backslash: a\.b stays a\.b */
	span:lang(a\.b) {
		font-style: italic;
	}
</style>
