<style>
	a {
		/* an escape is ident content, so an escape-spelled name is still a function */
		color: \66 n(0.1);
		border-color: v\61 r(--a, 0.1);
		/* the url recognizer decodes the name: quoted content takes the string path */
		background: u\72 l('a.png');
		/* unquoted url content stays opaque */
		mask-image: u\72 l(a.png);
		/* a hex escape spelling `(` is ident content like any other */
		outline-color: a\28 b(0.1);
		/* a raw escaped `(` leaves a dangling `\` at the name's end — not an ident
		sequence, so this is no function at all and stays verbatim */
		background-color: a\(b(.10);
	}
</style>
