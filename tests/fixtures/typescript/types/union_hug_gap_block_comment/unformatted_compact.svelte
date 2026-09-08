<script lang="ts">
	// A block comment glued between an operator and a should-hug union binds to the
	// FIRST MEMBER, so the union no longer hugs: it breaks after the operator, and
	// once the member overflows it takes the one-per-line form with the comment
	// after the pipe. The comment-free twin hugs the operator (`union_hug_*`).

	// Variable: 100 chars fits after the break, 101 takes the one-per-line form
	let a:/* c */{aaaa:Aaaaaaaaa;bbb:Bbbbbb;cccccccccccccccccccc:Cccccccccccccccccccccccc}|null;
	let b:/* c */{aaaa:Aaaaaaaaa;bbb:Bbbbbb;cccccccccccccccccccc:Ccccccccccccccccccccccccc}|null;
	let c:/* c */Map<string,numberrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrr>|null;
	let d:/* c */null|{aaaa:Aaaaaaaaa;bbb:Bbbbbb;cccccccccccccccccccc:Cccccccccccccccccccccccccc};

	// Class property and interface members
	class Single{a:/* c */{aaaa:Aaaaaaaaa;bbb:Bbbbbb;cccccccccccccccccccc:Cccccccccccccccccccccccccc}|null}
	interface Multiple{a:/* c */{aaaa:Aaaaaaaaa;bbb:Bbbbbb;cccccccccccccccccccc:Cccccccccccccccccccccccccccc}|null;[key:string]:/* c */{aaaa:Aaaaaaaaa;bbb:Bbbbbb;cccccccccc:Cccccccccccccccccccccccccc}|null}

	// Parameter, return type, and the function-type `=>`
	function param(a:/* c */{aaaa:Aaaaaaaaa;bbb:Bbbbbb;cccccccccccccccccccc:Cccccccccccccccc}|null,b:string){}
	function ret():/* c */{aaaa:Aaaaaaaaa;bbb:Bbbbbb;cccccccccccccccccccc:Cccccccccccccccccccccccccc}|null{}
	function retLong():/* c */{aaaa:Aaaaaaaaa;bbb:Bbbbbb;cccccccccccccccccccc:Cccccccccccccccccccccccccc;dddd:D}|null{}
	type A=()=>/* c */{aaaa:Aaaaaaaaa;bbb:Bbbbbb;cccccccccccccccccccc:Cccccccccccccccccccccccccc}|null;
	type B=()=>/* c */{aaaa:Aaaaaaaaa;bbb:Bbbbbb;cccccccccccccccccccc:Cccccccccccccccccccccccccc;dddd:D}|null;

	// Type alias and mapped-type value
	type C=/* c */{aaaa:Aaaaaaaaa;bbb:Bbbbbb;cccccccccccccccccccc:Cccccccccccccccccccccccccc}|null;
	type D={[K in T]:/* c */{aaaa:Aaaaaaaaa;bbb:Bbbbbb;cccccccccccccccccccc:Cccccccccccccccccccc}|null};

	// A type predicate binds the comment to its annotation instead, and keeps hugging
	function pred(a:unknown):a is/* c */{
aaaa:Aaaaaaaaa;
bbb:Bbbbbb;
cccccccccccccccc:Cccccccccccccccccccc;
dddd:D;
}|null{}

	// Everything fits: the comment stays glued and nothing breaks
	let e:/* c */{a:1}|null;
	type E=()=>/* c */{a:1}|null;
</script>
