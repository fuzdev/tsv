<script lang="ts">
	// A block comment glued ahead of a should-hug union's AUTHORED leading pipe binds
	// to the first member exactly like one glued after it: the union declines the hug,
	// drops the pipe where the member fits after the operator, and carries the comment
	// after the synthesized pipe once it takes the one-per-line form. A newline between
	// the pipe and the member changes nothing — the comment is glued to the pipe.
	// `unformatted_leading_pipe*.svelte` hold the authored spellings.

	// Variable: 100 chars fits after the break, 101 takes the one-per-line form
	let a: /* c */ |
		{ aaaa: Aaaaaaaaa; bbb: Bbbbbb; cccccccccccccccccccc: Cccccccccccccccccccccccc } | null;
	let b: /* c */ |
		{ aaaa: Aaaaaaaaa; bbb: Bbbbbb; cccccccccccccccccccc: Ccccccccccccccccccccccccc } | null;

	// Property, alias, function type, return type, parameter
	interface Multiple {
		a: /* c */ |
			{ aaaa: Aaaaaaaaa; bbb: Bbbbbb; cccccccccccccccccccc: Cccccccccccccccccccccccccccc } | null;
	}
	type C = /* c */ |
		{ aaaa: Aaaaaaaaa; bbb: Bbbbbb; cccccccccccccccccccc: Cccccccccccccccccccccccccc } | null;
	type D = () => /* c */ |
		{ aaaa: Aaaaaaaaa; bbb: Bbbbbb; cccccccccccccccccccc: Cccccccccccccccccccccccccc } | null;
	function ret(): /* c */ |
		{ aaaa: Aaaaaaaaa; bbb: Bbbbbb; cccccccccccccccccccc: Cccccccccccccccccccccccccc } | null {}
	function param(
		a: /* c */ |
			{ aaaa: Aaaaaaaaa; bbb: Bbbbbb; cccccccccccccccccccc: Cccccccccccccccccccccccccccccccc } | null,
		b: string
	) {}

	// A container and a keyword
	type E = Map<string, /* c */ |
		{ aaaa: Aaaaaaaaa; bbb: Bbbbbb; cccccccccccccccccccc: Cccccccccccccccccccccccccc } | null>;
	type F = T extends U ? /* c */ |
		{ aaaa: Aaaaaaaaa; bbb: Bbbbbb; cccccccccccccccccccc: Cccccccccccccccccccccccccccc } | null : 2;

	// An expanded member; two blocks around the pipe; a block and then a line comment
	let g: /* c */ |
		{ aaaa: Aaaaaaaaa; bbb: Bbbbbb; cccccccccccccccccccc: Cccccccccccccccccccccccccc; dddd: D; eeee: E } | null;
	let h: /* c1 */ |
		/* c2 */ { aaaa: Aaaaaaaaa; bbb: Bbbbbb; cccccccccccccccccccc: Ccccccccccccccccccccccccc } | null;
	let i: /* c */ | // d
		{ aaaa: Aaaaaaaaa; bbb: Bbbbbb; cccccccccccccccccccc: Cccccccccccccccccccccccccc } | null;

	// A single-member union is its member: the comment stays glued and the object hugs
	let j: /* c */ |
		{ aaaa: Aaaaaaaaa; bbb: Bbbbbb; cccccccccccccccccccc: Cccccccccccccccccccccccccccccccccccccc };

	// A type predicate binds the comment to its annotation and keeps hugging
	function pred(a: unknown): a is /* c */ |
		{
			aaaa: Aaaaaaaaa;
			bbb: Bbbbbb;
			cccccccccccccccc: Cccccccccccccccccccc;
			dddd: D;
		} | null {}

	// Everything fits: the pipe is dropped and nothing breaks
	let k: /* c */ |
		{ a: 1 } | null;
</script>
