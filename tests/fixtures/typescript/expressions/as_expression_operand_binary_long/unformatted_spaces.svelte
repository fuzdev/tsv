<script lang="ts">
	// 100 chars — the whole cast fits, the chain stays inline
	const a =   (  aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa   ??   bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb  )   as   Foo;

	// 101 chars — the chain breaks after `??`; the continuation indents one level past the statement
	const b =  ( aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa  ??  bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb )  as  Foo;

	// 101 chars — satisfies shares the cast path, same continuation indent
	const c =  ( aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa   ??   bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb )   satisfies   Foo;

	// 101 chars — three logical operands: every continuation line takes that same one level
	const d =  ( aaaaaaaaaaaaaaaaaaaaaaa  &&  bbbbbbbbbbbbbbbbbbbbbbbb  &&  cccccccccccccccccccccccc )  as  Foo;

	// 101 chars — arithmetic operands lay out the same way
	const e =  ( aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa  +  bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb )  as  Foo;

	// 101 chars one tab deeper — the indent is relative to the statement, not the left margin
	function fn()  {
		const f =  ( aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa  ??  bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb )  as  Foo;
	}
</script>
