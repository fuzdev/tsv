<script lang="ts">
	// A block comment glued ahead of a should-hug union's first member binds to that
	// member after a type keyword too — a type parameter's `extends` / `=`, a
	// conditional's `extends` / `?` / `:`, a mapped type's `in` — so the union no
	// longer hugs: it breaks after the keyword once the member no longer fits beside
	// it, and takes the one-per-line form with the comment after the pipe once the
	// member overflows the continuation line. A conditional's CHECK type is the
	// exception: the conditional starts where the union does, the comment binds to
	// it, and the union keeps hugging.

	// Type parameter bound: 100 chars stays on the declaration line, 101 breaks the
	// list; a parameter line of 101 breaks after `extends`, a continuation of 101
	// takes the one-per-line form
	function a<T extends/* c */{aaaa:Aaaaaaaaa;bbb:Bbbbbb;ccc:Ccccccccccccccccc}|null>(){}
	function b<T extends/* c */{aaaa:Aaaaaaaaa;bbb:Bbbbbb;ccc:Cccccccccccccccccc}|null>(){}
	function c<T extends/* c */{aaaa:Aaaaaaaaa;bbb:Bbbbbb;ccc:Ccccccccccccccccccccccccccccccccc}|null>(){}
	function d<T extends/* c */{aaaa:Aaaaaaaaa;bbb:Bbbbbb;ccc:Cccccccccccccccccccccccccccccccccccccccc}|null>(){}
	function e<T extends|/* c */{aaaa:Aaaaaaaaa;bbb:Bbbbbb;ccc:Ccccccccccccccccccccccccccccccccccccccccc}|null>(){}
	function f<T=|/* c */{aaaa:Aaaaaaaaa;bbb:Bbbbbb;cccccccccccccccccccc:Cccccccccccccccccccccccccccc}|null>(){}

	// Conditional keywords: the `extends` type, and either branch. The comment-free
	// twin keeps `extends {` glued with the member owning its expansion
	type O=T extends{aaaa:Aaaaaaaaa;bbb:Bbbbbb;cccccccccccccccccccc:Cccccccccccccccccccccccccccccc;}|null?1:2;
	type G=T extends|/* c */{aaaa:Aaaaaaaaa;bbb:Bbbbbb;cccccccccccccccccccc:Cccccccccccccccccccccccc;dddd:D;}
	|null?1:2;
	type H=T extends U?/* c */{aaaa:Aaaaaaaaa;bbb:Bbbbbb;cccccccccccccccccccc:Ccccccccccccccccccccccc}|null:2;
	type I=T extends U?1:|/* c */{aaaa:Aaaaaaaaa;bbb:Bbbbbb;cccccccccccccccccccc:Ccccccccccccccccccccccc}
	|null;
	type J=T extends U?|/* c */{aaaa:Aaaaaaaaa;bbb:Bbbbbb;cccccccccccccccccccc:Cccccccccccccccccccccccccccc}
	|null:2;
	type K=T extends U?1:|/* c */{aaaa:Aaaaaaaaa;bbb:Bbbbbb;cccccccccccccccccccc:Cccccccccccccccccccccccccccc}
	|null;

	// Mapped `in`
	type L={[K in|/* c */{aaaa:Aaaaaaaaa;bbb:Bbbbbb;cccccccccccccccccccc:Cccccccccccccccccccccccc}|null]:1;};
	type M={[K in|/* c */{aaaa:Aaaaaaaaa;bbb:Bbbbbb;cccccccccccccccccccc:Cccccccccccccccccccccccccccccc;}|null]:1;};

	// The check type keeps hugging
	type N=/* c */{aaaa:Aaaaaaaaa;bbb:Bbbbbb;cccccccccccccccccccc:Cccccccccccccccccccccccccccccc;}|null extends U?1:2;
</script>
