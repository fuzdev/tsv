<script lang="ts">
	// A nested conditional in the TRUE branch prints clarity parens while it fits, so
	// a comment the author wrote inside that shell stays inside it
	type A = T extends U ? (/* c */ V extends W ? X : Y) : Z;
	type B = T extends U ? (V extends W ? X : Y /* c */) : Z;
	type C = T extends U ? (/* c1 */ V extends W ? X : Y /* c2 */) : Z;

	// A redundant double shell collapses to the one pair the branch prints
	type D = T extends U ? ((/* c */ V extends W ? X : Y)) : Z;

	// The FALSE branch is right-associative and takes no parens, so the shell strips
	// and its comments lead / trail the branch itself
	type E = T extends U ? Z : (/* c */ V extends W ? X : Y);
	type F = T extends U ? Z : (V extends W ? X : Y /* c */);

	// Broken by width, the clarity parens go away and the true branch reads like the false one
	type G = Taaaaaaaaaaaaaaaaaaaaaa extends Uaaaaaaaaaaaaaaaaaaaaaa ? (/* c */ Vaaaaaaaaaaaaaaaaaaaaaa extends Waaaaaaaaaaaaaaaaaaaaaa ? Xaaaa : Yaaaa) : Zaaaa;
	type H = Taaaaaaaaaaaaaaaaaaaaaa extends Uaaaaaaaaaaaaaaaaaaaaaa ? (Vaaaaaaaaaaaaaaaaaaaaaa extends Waaaaaaaaaaaaaaaaaaaaaa ? Xaaaa : Yaaaa /* c */) : Zaaaa;

	// A LINE comment in the shell's leading gap ends its line, so it trails what
	// precedes the branch and the nested conditional breaks below it
	type I = T extends U ? (// c
	V extends W ? X : Y) : Z;
	type J = T extends U ? Z : (// c
	V extends W ? X : Y);

	// A line comment in the trailing gap stays on the branch it was written on
	type K = T extends U ? (V extends W ? X : Y // c
	) : Z;

	// Control: with no comment the shell strips at every width and the nested branch
	// sits exactly one level past its operator
	type L = T extends U ? (V extends W ? X : Y) : Z;
</script>
