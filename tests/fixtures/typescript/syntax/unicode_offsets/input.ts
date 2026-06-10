// 2-byte (é), 3-byte (中), and astral 4-byte (😀 = 2 UTF-16 code units) characters
// shift byte offsets away from UTF-16 offsets for everything that follows.
const café = 'é';
const a = '中文',
	b = '😀',
	c = 2;
const 日本語 = `pre 😀 ${a} post ${c}`;
/* astral ZWJ sequence 👨‍👩‍👧 on the same line */ const d = /中😀/u;
const e = 4;
// a broken type-param list keeps its trailing comma — its position
// (extra.trailingComma) sits past the multibyte chars above and must translate
const fn = <
	Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
	Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb,
>(
	x: Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
) => x;
