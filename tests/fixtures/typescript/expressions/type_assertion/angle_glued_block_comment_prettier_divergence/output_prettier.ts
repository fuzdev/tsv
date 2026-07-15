// A glued pair of blocks after the `<`-line comment stays glued.
const a = <
	// force
	/* c1 */ /* c2 */
	Aaaaaaaa
>xxx;

// Blocks the author put on their own lines keep them.
const b = <
	// force
	/* c1 */
	/* c2 */
	Bbbbbbbb
>yyy;
