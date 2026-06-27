<script lang="ts">
	// ECMAScript identifiers use the Unicode ID_Start / ID_Continue properties,
	// not the NFKC-closed XID_Start / XID_Continue variants
	// (ecma262 §sec-names-and-keywords → UAX #31). These code points are in ID_*
	// but excluded from XID_*, so a parser keyed on XID alone wrongly rejects them.

	// Other_ID_Start voiced/semi-voiced sound marks:
	const ゛ = 1; // U+309B KATAKANA-HIRAGANA VOICED SOUND MARK
	const ゜ = 2; // U+309C KATAKANA-HIRAGANA SEMI-VOICED SOUND MARK

	// Letters whose NFKC decomposition leaves the identifier set:
	const ͺ = 3; // U+037A GREEK YPOGEGRAMMENI
	const ำ = 4; // U+0E33 THAI CHARACTER SARA AM
	const ຳ = 5; // U+0EB3 LAO VOWEL SIGN AM
	const ﾞ = 6; // U+FF9E HALFWIDTH KATAKANA VOICED SOUND MARK
	const ﾟ = 7; // U+FF9F HALFWIDTH KATAKANA SEMI-VOICED SOUND MARK

	// Arabic ligature and presentation forms (Lo letters that NFKC-decompose):
	const ﷺ = 8; // U+FDFA ARABIC LIGATURE SALLALLAHOU ALAYHE WASALLAM
	const ﹰ = 9; // U+FE70 ARABIC FATHATAN ISOLATED FORM
	const ﱞ = 10; // U+FC5E ARABIC LIGATURE SHADDA WITH DAMMATAN ISOLATED FORM

	// Also valid in continue position, after an ASCII start char:
	const a゛ = 11; // U+309B as a continue char
	const aͺ = 12; // U+037A as a continue char
</script>
