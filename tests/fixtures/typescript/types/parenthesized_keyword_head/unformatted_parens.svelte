<script lang="ts">
	// A parenthesized type whose first token is a keyword — a literal, `this`, or
	// a reserved word standing as a type name — is a type, not a parameter list

	// Boolean literals
	type A1 = (true | false)[];
	type A2 = (false & A)[];

	// `this`
	type B1 = (this | A)[];
	type B2 = (this & A)[K];

	// Reserved and contextual words as type names
	type C1 = (default | A)[];
	type C2 = (function | A)[];
	type C3 = (class & A)[];
	type C4 = (var | A)[];
	type C5 = (async | A)[];
	type C6 = (await | A)[];
	type C7 = (yield | A)[];
	type C8 = (let | A)[];

	// Redundant parens are stripped
	type D1 = (true);
	type D2 = (this[K]);
	type D3 = (break);
	type D4 = (case<T>);
	type D5 = (if.a);
	type D6 = (const);
	type D7 = (enum);
	type D8 = (super);
	type D9 = (in);
</script>
